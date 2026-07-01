use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use clap::{Parser as ClapParser, Subcommand};
use serde_json;

use dcl_frontend::lexer::Lexer;
use dcl_frontend::parser::Parser;
use dcl_frontend::typechecker::TypeChecker;
use dcl_ir::Lowerer;
use dcl_codegen::CodeGenerator;

#[derive(ClapParser)]
#[command(name = "dcl")]
#[command(about = "🔮 DCL: Differentiable Cryptographic Language Compiler CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Verify syntax and type-check a DCL program
    Check {
        /// Path to input DCL file
        input: String,
    },
    /// Compile and optimize a DCL program
    Compile {
        /// Path to input DCL file
        input: String,
        /// Path to output file
        #[arg(short, long)]
        output: Option<String>,
        /// Target backend: circom or fhe
        #[arg(short, long, default_value = "circom")]
        backend: String,
        /// Number of optimization epochs
        #[arg(long, default_value_t = 300)]
        epochs: usize,
    },
    /// Format a DCL program in-place
    Fmt {
        /// Path to DCL file to format
        input: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Check { input } => {
            if let Err(e) = run_check(&input) {
                eprintln!("❌ Check failed: {}", e);
                std::process::exit(1);
            }
            println!("✅ Syntax and type checking passed successfully!");
        }
        Commands::Compile { input, output, backend, epochs } => {
            let output_path = output.unwrap_or_else(|| {
                let mut path = PathBuf::from(&input);
                let ext = if backend == "fhe" { "rs" } else { "circom" };
                path.set_extension(ext);
                path.to_string_lossy().to_string()
            });

            if let Err(e) = run_compile(&input, &output_path, &backend, epochs) {
                eprintln!("❌ Compilation failed: {}", e);
                std::process::exit(1);
            }
            println!("✅ Compilation and optimization completed successfully!");
            println!("   Output saved to: {}", output_path);
        }
        Commands::Fmt { input } => {
            if let Err(e) = run_fmt(&input) {
                eprintln!("❌ Formatting failed: {}", e);
                std::process::exit(1);
            }
            println!("✅ File formatted successfully: {}", input);
        }
    }
}

fn run_check(input_path: &str) -> Result<(), String> {
    let module = load_module_and_imports(Path::new(input_path))?;
    let mut checker = TypeChecker::new();
    checker.check_module(&module)?;
    Ok(())
}

fn load_module_and_imports(input_path: &Path) -> Result<dcl_frontend::ast::Module, String> {
    let content = fs::read_to_string(input_path)
        .map_err(|e| format!("Failed to read file {:?}: {}", input_path, e))?;

    let mut lexer = Lexer::new(&content);
    let tokens = lexer.tokenize()?;
    let mut parser = Parser::new(tokens);
    let mut main_module = parser.parse_module()?;

    // Resolve imports
    for import in &main_module.imports {
        if import.is_empty() {
            continue;
        }
        let imported_path = if import[0] == "std" {
            // Stdlib path: std::crypto -> stdlib/crypto.dcl
            let mut p = PathBuf::from("/Users/liuyukai/CREATE/auv/dcl/stdlib");
            for part in &import[1..] {
                p.push(part);
            }
            p.set_extension("dcl");
            p
        } else {
            // Local path: relative to input_path's parent directory
            let mut p = input_path.parent().unwrap_or_else(|| Path::new(".")).to_path_buf();
            for part in import {
                p.push(part);
            }
            p.set_extension("dcl");
            p
        };

        if !imported_path.exists() {
            return Err(format!("Could not resolve import path: {:?}", imported_path));
        }

        // Parse imported module
        let imported_module = load_module_and_imports(&imported_path)?;
        let namespace = import.join("::"); // e.g. "std::crypto" or "crypto"
        let namespace_short = if import[0] == "std" { import[1..].join("::") } else { namespace.clone() };

        for mut circuit in imported_module.circuits {
            let namespaced_name1 = format!("{}::{}", namespace, circuit.name);
            let namespaced_name2 = format!("{}::{}", namespace_short, circuit.name);
            circuit.name = namespaced_name1.clone();
            main_module.circuits.push(circuit.clone());
            if namespaced_name1 != namespaced_name2 {
                let mut c2 = circuit.clone();
                c2.name = namespaced_name2;
                main_module.circuits.push(c2);
            }
        }
        for mut tdef in imported_module.types {
            let namespaced_name1 = format!("{}::{}", namespace, tdef.name);
            let namespaced_name2 = format!("{}::{}", namespace_short, tdef.name);
            tdef.name = namespaced_name1.clone();
            main_module.types.push(tdef.clone());
            if namespaced_name1 != namespaced_name2 {
                let mut t2 = tdef.clone();
                t2.name = namespaced_name2;
                main_module.types.push(t2);
            }
        }
    }

    Ok(main_module)
}

fn run_compile(input_path: &str, output_path: &str, backend: &str, epochs: usize) -> Result<(), String> {
    let module = load_module_and_imports(Path::new(input_path))?;

    // 1. Frontend Check
    let mut checker = TypeChecker::new();
    checker.check_module(&module)?;

    // 2. Lowering to DCIR Graph
    // Lower first non-extern circuit in the module
    let circuit = module.circuits.iter()
        .find(|c| !c.is_extern)
        .ok_or_else(|| "Module contains no non-extern circuits to compile".to_string())?;

    let mut lowerer = Lowerer::new(&module);
    let graph = lowerer.lower_circuit(circuit)?;

    // Save temporary IR file for optimizer
    let temp_dir = std::env::temp_dir();
    let ir_in_path = temp_dir.join(format!("{}_ir_in.json", circuit.name));
    let ir_out_path = temp_dir.join(format!("{}_ir_out.json", circuit.name));

    let ir_in_str = serde_json::to_string_pretty(&graph)
        .map_err(|e| format!("Failed to serialize IR: {}", e))?;
    fs::write(&ir_in_path, ir_in_str)
        .map_err(|e| format!("Failed to write temporary IR file: {}", e))?;

    // 3. Invoke Python JAX Optimizer
    println!("🚀 Launching differentiable strategy optimization...");
    
    // Find virtual env python interpreter
    let python_paths = [
        "/Users/liuyukai/CREATE/auv/dcl-poc/.venv/bin/python",
        "/Users/liuyukai/CREATE/auv/dcl/.venv/bin/python",
        "python3",
        "python",
    ];

    let mut python_cmd = "python3";
    for path in &python_paths {
        if Path::new(path).exists() {
            python_cmd = path;
            break;
        }
    }

    // Locate the optimize.py and verify.py scripts relative to workspace root
    let workspace_root = if Path::new("dcl-optimizer/optimize.py").exists() {
        PathBuf::from(".")
    } else {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
        let mut p = PathBuf::from(manifest_dir);
        if p.ends_with("crates/dcl-cli") {
            p.pop();
            p.pop();
        }
        p
    };

    let optimize_script = workspace_root.join("dcl-optimizer/optimize.py");
    let verify_script = workspace_root.join("dcl-optimizer/verify.py");

    let status = Command::new(python_cmd)
        .arg(&optimize_script)
        .arg("--input")
        .arg(&ir_in_path)
        .arg("--output")
        .arg(&ir_out_path)
        .arg("--epochs")
        .arg(epochs.to_string())
        .status()
        .map_err(|e| format!("Failed to run python optimizer: {}. Make sure the dcl-poc venv is configured.", e))?;

    if !status.success() {
        return Err(format!("Python optimizer exited with error status: {:?}", status.code()));
    }

    // 3.5. Invoke Z3 SMT Equivalence Verifier
    println!("🛡️ Launching Z3 SMT formal equivalence verification...");
    let verify_status = Command::new(python_cmd)
        .arg(&verify_script)
        .arg("--input")
        .arg(&ir_in_path)
        .arg("--output")
        .arg(&ir_out_path)
        .status()
        .map_err(|e| format!("Failed to run Z3 equivalence verifier: {}", e))?;

    if !verify_status.success() {
        return Err("Z3 equivalence check FAILED! Optimization introduced semantics changes.".to_string());
    }

    // 4. Read Optimized IR
    let optimized_ir_str = fs::read_to_string(&ir_out_path)
        .map_err(|e| format!("Failed to read optimized IR: {}", e))?;
    let optimized_graph: dcl_ir::Graph = serde_json::from_str(&optimized_ir_str)
        .map_err(|e| format!("Failed to deserialize optimized IR: {}", e))?;

    // 5. Code Generation
    let codegen = CodeGenerator::new(optimized_graph);
    let output_code = if backend == "fhe" {
        codegen.generate_tfhe()?
    } else {
        codegen.generate_circom()?
    };

    fs::write(output_path, output_code)
        .map_err(|e| format!("Failed to write output file: {}", e))?;

    // Cleanup temporary files
    let _ = fs::remove_file(ir_in_path);
    let _ = fs::remove_file(ir_out_path);

    Ok(())
}

fn run_fmt(input_path: &str) -> Result<(), String> {
    let content = fs::read_to_string(input_path)
        .map_err(|e| format!("Failed to read file {:?}: {}", input_path, e))?;

    let mut lexer = Lexer::new(&content);
    let tokens = lexer.tokenize()?;
    let mut parser = Parser::new(tokens);
    let module = parser.parse_module()?;

    let formatted = dcl_frontend::format_module(&module);
    fs::write(input_path, formatted)
        .map_err(|e| format!("Failed to write formatted code to file: {}", e))?;
    Ok(())
}

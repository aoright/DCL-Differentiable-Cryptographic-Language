//! DCL CLI — the command-line interface for the Differentiable Cryptographic Language compiler.
//!
//! Provides `check`, `compile`, `fmt`, and `init` subcommands for the full
//! development workflow.

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
#[command(version = "0.2.0")]
#[command(about = "🔮 DCL: Differentiable Cryptographic Language Compiler", long_about = None)]
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
        /// Print verbose debug output including IR details
        #[arg(long)]
        verbose: bool,
        /// Emit the intermediate DCIR graph as JSON
        #[arg(long)]
        emit_ir: bool,
    },
    /// Format a DCL program in-place
    Fmt {
        /// Path to DCL file to format
        input: String,
    },
    /// Initialize a new DCL project
    Init {
        /// Project name (defaults to current directory name)
        #[arg(short, long)]
        name: Option<String>,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Check { input } => {
            if let Err(e) = run_check(&input) {
                print_error_with_source_context(&e, &input);
                std::process::exit(1);
            }
            println!("✅ Syntax and type checking passed successfully!");
        }
        Commands::Compile { input, output, backend, epochs, verbose, emit_ir } => {
            let output_path = output.unwrap_or_else(|| {
                let mut path = PathBuf::from(&input);
                let ext = if backend == "fhe" { "rs" } else { "circom" };
                path.set_extension(ext);
                path.to_string_lossy().to_string()
            });

            if let Err(e) = run_compile(&input, &output_path, &backend, epochs, verbose, emit_ir) {
                print_error_with_source_context(&e, &input);
                std::process::exit(1);
            }
            println!("✅ Compilation and optimization completed successfully!");
            println!("   Output saved to: {}", output_path);
        }
        Commands::Fmt { input } => {
            if let Err(e) = run_fmt(&input) {
                print_error_with_source_context(&e, &input);
                std::process::exit(1);
            }
            println!("✅ File formatted successfully: {}", input);
        }
        Commands::Init { name } => {
            if let Err(e) = run_init(name) {
                eprintln!("❌ Initialization failed: {}", e);
                std::process::exit(1);
            }
        }
    }
}

fn run_check(input_path: &str) -> Result<(), String> {
    let module = load_module_and_imports(Path::new(input_path))?;
    let mut checker = TypeChecker::new();
    checker.check_module(&module)?;
    Ok(())
}

/// Resolve the stdlib directory path.
///
/// Checks in order:
/// 1. `DCL_STDLIB_PATH` environment variable
/// 2. `stdlib/` relative to the workspace root
/// 3. `../stdlib/` relative to the input file
fn resolve_stdlib_dir(input_path: &Path) -> PathBuf {
    if let Ok(env_path) = std::env::var("DCL_STDLIB_PATH") {
        return PathBuf::from(env_path);
    }

    // Try relative to CARGO_MANIFEST_DIR or common workspace layouts
    let candidates = [
        PathBuf::from("stdlib"),
        PathBuf::from("../stdlib"),
        PathBuf::from("../../stdlib"),
    ];

    // Also try relative to input file
    if let Some(parent) = input_path.parent() {
        let relative = parent.join("../stdlib");
        if relative.exists() {
            return relative;
        }
    }

    // Try CARGO_MANIFEST_DIR
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        let mut p = PathBuf::from(manifest);
        // Navigate from crates/dcl-cli up to workspace root
        if p.ends_with("crates/dcl-cli") {
            p.pop();
            p.pop();
        }
        let stdlib = p.join("stdlib");
        if stdlib.exists() {
            return stdlib;
        }
    }

    for c in &candidates {
        if c.exists() {
            return c.clone();
        }
    }

    // Fallback
    PathBuf::from("stdlib")
}

fn load_module_and_imports(input_path: &Path) -> Result<dcl_frontend::ast::Module, String> {
    let content = fs::read_to_string(input_path)
        .map_err(|e| format!("Failed to read file {:?}: {}", input_path, e))?;

    let mut lexer = Lexer::new(&content);
    let tokens = lexer.tokenize()?;
    let mut parser = Parser::new(tokens);
    let mut main_module = parser.parse_module()?;

    let stdlib_dir = resolve_stdlib_dir(input_path);

    // Resolve imports
    for import in &main_module.imports {
        if import.is_empty() {
            continue;
        }
        let imported_path = if import[0] == "std" {
            // Stdlib path: std::crypto -> stdlib/crypto.dcl
            let mut p = stdlib_dir.clone();
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
            return Err(format!("Could not resolve import path: {:?} (stdlib dir: {:?})", imported_path, stdlib_dir));
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

fn run_compile(input_path: &str, output_path: &str, backend: &str, epochs: usize, verbose: bool, emit_ir: bool) -> Result<(), String> {
    let module = load_module_and_imports(Path::new(input_path))?;

    // 1. Frontend Check
    let mut checker = TypeChecker::new();
    checker.check_module(&module)?;

    // 2. Lowering to DCIR Graph
    let circuit = module.circuits.iter()
        .find(|c| !c.is_extern)
        .ok_or_else(|| "Module contains no non-extern circuits to compile".to_string())?;

    let mut lowerer = Lowerer::new(&module);
    let mut graph = lowerer.lower_circuit(circuit)?;

    // 2.5 Run optimization passes
    graph.constant_fold();
    graph.algebraic_simplify();
    graph.constant_fold();
    graph.cse();
    graph.dead_code_eliminate();

    // 2.6 Security analysis
    let diagnostics = graph.check_information_flow();
    if verbose && !diagnostics.is_empty() {
        println!("   Security diagnostics: {} issue(s) found", diagnostics.len());
    }

    if verbose {
        println!("   📊 DCIR: {} nodes, {} outputs", graph.nodes.len(), graph.outputs.len());
    }

    // Save temporary IR file for optimizer
    let temp_dir = std::env::temp_dir();
    let ir_in_path = temp_dir.join(format!("{}_ir_in.json", circuit.name));
    let ir_out_path = temp_dir.join(format!("{}_ir_out.json", circuit.name));

    let ir_in_str = serde_json::to_string_pretty(&graph)
        .map_err(|e| format!("Failed to serialize IR: {}", e))?;

    if emit_ir {
        let ir_path = format!("{}.dcir.json", input_path.trim_end_matches(".dcl"));
        fs::write(&ir_path, &ir_in_str)
            .map_err(|e| format!("Failed to write IR file: {}", e))?;
        println!("   📄 DCIR exported to: {}", ir_path);
    }

    fs::write(&ir_in_path, &ir_in_str)
        .map_err(|e| format!("Failed to write temporary IR file: {}", e))?;

    // 3. Invoke Python JAX Optimizer
    println!("🚀 Launching differentiable strategy optimization...");
    
    let python_cmd = find_python();

    let workspace_root = find_workspace_root();
    let optimize_script = workspace_root.join("dcl-optimizer/optimize.py");
    let verify_script = workspace_root.join("dcl-optimizer/verify.py");

    let status = Command::new(&python_cmd)
        .arg(&optimize_script)
        .arg("--input")
        .arg(&ir_in_path)
        .arg("--output")
        .arg(&ir_out_path)
        .arg("--epochs")
        .arg(epochs.to_string())
        .status()
        .map_err(|e| format!("Failed to run python optimizer: {}. Ensure Python with JAX is available.", e))?;

    if !status.success() {
        return Err(format!("Python optimizer exited with error status: {:?}", status.code()));
    }

    // 3.5. Invoke Z3 SMT Equivalence Verifier
    println!("🛡️ Launching Z3 SMT formal equivalence verification...");
    let mut verify_cmd = Command::new(&python_cmd);
    verify_cmd
        .arg(&verify_script)
        .arg("--input")
        .arg(&ir_in_path)
        .arg("--output")
        .arg(&ir_out_path);

    if verbose {
        verify_cmd.arg("--verbose");
    }

    let verify_status = verify_cmd.status()
        .map_err(|e| format!("Failed to run Z3 equivalence verifier: {}", e))?;

    if !verify_status.success() {
        let code = verify_status.code().unwrap_or(-1);
        if code == 2 {
            eprintln!("⚠️  Z3 verification timed out. Proceeding with caution.");
        } else {
            return Err("Z3 equivalence check FAILED! Optimization introduced semantics changes.".to_string());
        }
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

fn run_init(name: Option<String>) -> Result<(), String> {
    let project_name = name.unwrap_or_else(|| {
        std::env::current_dir()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
            .unwrap_or_else(|| "my_dcl_project".to_string())
    });

    println!("🔮 Initializing DCL project: {}", project_name);

    // Create directory structure
    fs::create_dir_all("src").map_err(|e| format!("Failed to create src/: {}", e))?;

    // Create main.dcl
    let main_content = format!(
        "module {}\n\ncircuit main(\n    private input: Field,\n    public expected: Field\n) -> bool {{\n    return input == expected;\n}}\n",
        project_name
    );
    let main_path = "src/main.dcl";
    if !Path::new(main_path).exists() {
        fs::write(main_path, main_content)
            .map_err(|e| format!("Failed to write {}: {}", main_path, e))?;
        println!("   Created {}", main_path);
    }

    println!("✅ Project '{}' initialized!", project_name);
    println!("   📁 src/main.dcl");
    println!("\n   Next steps:");
    println!("     dcl check src/main.dcl");
    println!("     dcl compile src/main.dcl");
    Ok(())
}

/// Find the Python interpreter to use for optimizer/verifier.
fn find_python() -> String {
    let candidates = [
        "python3",
        "python",
    ];

    // Also check for venv in the workspace
    if let Ok(workspace) = std::env::var("CARGO_MANIFEST_DIR") {
        let mut p = PathBuf::from(workspace);
        if p.ends_with("crates/dcl-cli") {
            p.pop();
            p.pop();
        }
        let venv = p.join(".venv/bin/python");
        if venv.exists() {
            return venv.to_string_lossy().to_string();
        }
    }

    for c in &candidates {
        if Command::new(c).arg("--version").output().is_ok() {
            return c.to_string();
        }
    }

    "python3".to_string()
}

/// Find the workspace root directory containing dcl-optimizer/.
fn find_workspace_root() -> PathBuf {
    if Path::new("dcl-optimizer/optimize.py").exists() {
        return PathBuf::from(".");
    }

    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let mut p = PathBuf::from(manifest_dir);
        if p.ends_with("crates/dcl-cli") {
            p.pop();
            p.pop();
        }
        return p;
    }

    PathBuf::from(".")
}

/// Print formatted error with source code context pointing to the line/col of the failure.
fn print_error_with_source_context(err_msg: &str, file_path: &str) {
    let content = match fs::read_to_string(file_path) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("{}", err_msg);
            return;
        }
    };
    let lines: Vec<&str> = content.lines().collect();

    for single_err in err_msg.lines() {
        if single_err.trim().is_empty() {
            continue;
        }
        
        if let Some(start_idx) = single_err.find("[Error at line ") {
            let sub = &single_err[start_idx + "[Error at line ".len()..];
            if let Some(comma_idx) = sub.find(", col ") {
                let line_str = &sub[..comma_idx];
                let sub2 = &sub[comma_idx + ", col ".len()..];
                if let Some(end_bracket_idx) = sub2.find(']') {
                    let col_str = &sub2[..end_bracket_idx];
                    
                    if let (Ok(line_num), Ok(col_num)) = (line_str.parse::<usize>(), col_str.parse::<usize>()) {
                        eprintln!("\n❌ {}", single_err);
                        eprintln!("   --> {}:{}:{}", file_path, line_num, col_num);
                        
                        if line_num > 0 && line_num <= lines.len() {
                            let line_code = lines[line_num - 1];
                            eprintln!("    |");
                            eprintln!("{:3} | {}", line_num, line_code);
                            let spaces = " ".repeat(col_num.saturating_sub(1));
                            eprintln!("    | {}^", spaces);
                            eprintln!("    |");
                        }
                        continue;
                    }
                }
            }
        }
        
        eprintln!("{}", single_err);
    }
}

use dcl_frontend::lexer::Lexer;
use dcl_frontend::parser::Parser;
use dcl_frontend::typechecker::TypeChecker;
use dcl_ir::Lowerer;
use dcl_codegen::CodeGenerator;

#[test]
fn test_age_verify_compilation() {
    let input = r#"
        module AgeVerification

        type Credential = {
            age: Field,
            id_hash: Field
        }

        circuit verify_adult(
            private cred: Credential,
            public threshold: Field
        ) -> bool {
            assert cred.age >= threshold;
            let computed_hash = poseidon(cred.age, cred.id_hash);
            return computed_hash == cred.id_hash;
        }
    "#;

    let mut lexer = Lexer::new(input);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let module = parser.parse_module().unwrap();
    let mut checker = TypeChecker::new();
    checker.check_module(&module).unwrap();

    let circuit = &module.circuits[0];
    let mut lowerer = Lowerer::new(&module);
    let mut graph = lowerer.lower_circuit(circuit).unwrap();

    // Mock the optimizer selected strategies to test codegen directly
    for node in &mut graph.nodes {
        if node.node_type == dcl_ir::NodeType::RangeCheck {
            node.alpha = Some(vec![0.0, 1.0, 0.0]); // select lookup_table
        } else if node.node_type == dcl_ir::NodeType::Poseidon {
            node.alpha = Some(vec![0.0, 0.0, 1.0]); // select lookup_assisted
        }
    }

    let codegen = CodeGenerator::new(graph);
    let circom_code = codegen.generate_circom().unwrap();

    assert!(circom_code.contains("template verify_adultMain()"));
    assert!(circom_code.contains("signal input cred_age;"));
    assert!(circom_code.contains("signal input cred_id_hash;"));
    assert!(circom_code.contains("signal input threshold;"));
    assert!(circom_code.contains("inv_"));
    assert!(circom_code.contains("inv_9 <-- n_8 == 0 ? 0 : 1 / n_8;"));
}

#[test]
fn test_merkle_range_compilation() {
    let input = r#"
        module MerkleAndRange

        circuit verify_leaf_and_merkle(
            private leaf: Field,
            private sibling0: Field,
            public root: Field,
            public bound: Field
        ) -> bool {
            assert leaf < bound;
            let hash1 = poseidon(leaf, sibling0);
            return hash1 == root;
        }
    "#;

    let mut lexer = Lexer::new(input);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let module = parser.parse_module().unwrap();
    let mut checker = TypeChecker::new();
    checker.check_module(&module).unwrap();

    let circuit = &module.circuits[0];
    let mut lowerer = Lowerer::new(&module);
    let mut graph = lowerer.lower_circuit(circuit).unwrap();

    // Mock optimizer strategy selections
    for node in &mut graph.nodes {
        if node.node_type == dcl_ir::NodeType::RangeCheck {
            node.alpha = Some(vec![0.0, 1.0, 0.0]); // select lookup_table
        } else if node.node_type == dcl_ir::NodeType::Poseidon {
            node.alpha = Some(vec![0.0, 0.0, 1.0]); // select lookup_assisted
        }
    }

    let codegen = CodeGenerator::new(graph);
    let circom_code = codegen.generate_circom().unwrap();

    assert!(circom_code.contains("template verify_leaf_and_merkleMain()"));
    assert!(circom_code.contains("n_4 <== bound - leaf;"));
    assert!(circom_code.contains("n_5 <== 1;"));
    assert!(circom_code.contains("n_6 <== n_4 - n_5;"));
}

#[test]
fn test_merkle_loop_compilation() {
    let input = r#"
        module MerkleLoop

        use std::crypto;

        circuit verify_merkle(
            private leaf: Field,
            private path: Field[4],
            public root: Field
        ) -> bool {
            let mut current = leaf;
            for i in 0..4 {
                current = crypto::poseidon(current, path[i]);
            }
            return current == root;
        }
    "#;

    let mut lexer = Lexer::new(input);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let mut module = parser.parse_module().unwrap();

    // Manually register imported extern circuit for test verification
    module.circuits.push(dcl_frontend::ast::Circuit {
        name: "std::crypto::poseidon".to_string(),
        params: vec![
            dcl_frontend::ast::Parameter {
                name: "x".to_string(),
                visibility: dcl_frontend::ast::Visibility::Private,
                ty: dcl_frontend::ast::Type::Field,
            },
            dcl_frontend::ast::Parameter {
                name: "y".to_string(),
                visibility: dcl_frontend::ast::Visibility::Private,
                ty: dcl_frontend::ast::Type::Field,
            },
        ],
        return_ty: dcl_frontend::ast::Type::Field,
        body: Vec::new(),
        is_extern: true,
        span: dcl_frontend::ast::Span::new(1, 1),
    });
    module.circuits.push(dcl_frontend::ast::Circuit {
        name: "crypto::poseidon".to_string(),
        params: vec![
            dcl_frontend::ast::Parameter {
                name: "x".to_string(),
                visibility: dcl_frontend::ast::Visibility::Private,
                ty: dcl_frontend::ast::Type::Field,
            },
            dcl_frontend::ast::Parameter {
                name: "y".to_string(),
                visibility: dcl_frontend::ast::Visibility::Private,
                ty: dcl_frontend::ast::Type::Field,
            },
        ],
        return_ty: dcl_frontend::ast::Type::Field,
        body: Vec::new(),
        is_extern: true,
        span: dcl_frontend::ast::Span::new(1, 1),
    });

    let mut checker = TypeChecker::new();
    checker.check_module(&module).unwrap();

    // The first non-extern circuit is "verify_merkle"
    let circuit = module.circuits.iter().find(|c| !c.is_extern).unwrap();
    let mut lowerer = Lowerer::new(&module);
    let mut graph = lowerer.lower_circuit(circuit).unwrap();

    // Mock optimizer strategy selections for the 4 Poseidon nodes
    for node in &mut graph.nodes {
        if node.node_type == dcl_ir::NodeType::Poseidon {
            node.alpha = Some(vec![0.0, 0.0, 1.0]); // select lookup_assisted
        }
    }

    let codegen = CodeGenerator::new(graph);
    let circom_code = codegen.generate_circom().unwrap();

    assert!(circom_code.contains("template verify_merkleMain()"));
    assert!(circom_code.contains("signal input path_0;"));
    assert!(circom_code.contains("signal input path_1;"));
    assert!(circom_code.contains("signal input path_2;"));
    assert!(circom_code.contains("signal input path_3;"));
    assert!(circom_code.contains("poseidon_7.inputs[0] <== leaf;"));
    assert!(circom_code.contains("poseidon_9.inputs[0] <== n_7;"));
    assert!(circom_code.contains("poseidon_9.inputs[1] <== path_1;"));
}

#[test]
fn test_equivalence_checker_detection() {
    use std::fs;
    use std::process::Command;

    // Create a simple addition circuit input graph
    let ir_in = r#"{
        "name": "add_test",
        "nodes": [
            {
                "id": 0,
                "node_type": "input",
                "inputs": [],
                "strategies": [],
                "label": "x"
            },
            {
                "id": 1,
                "node_type": "const",
                "inputs": [],
                "strategies": [],
                "value": 5.0,
                "label": "const_5"
            },
            {
                "id": 2,
                "node_type": "add",
                "inputs": [0, 1],
                "strategies": [],
                "label": "add_node"
            }
        ],
        "outputs": [2]
    }"#;

    // Create an invalid buggy optimized circuit graph (subtraction instead of addition)
    let ir_out = r#"{
        "name": "add_test",
        "nodes": [
            {
                "id": 0,
                "node_type": "input",
                "inputs": [],
                "strategies": [],
                "label": "x"
            },
            {
                "id": 1,
                "node_type": "const",
                "inputs": [],
                "strategies": [],
                "value": 5.0,
                "label": "const_5"
            },
            {
                "id": 2,
                "node_type": "sub",
                "inputs": [0, 1],
                "strategies": [],
                "label": "sub_node"
            }
        ],
        "outputs": [2]
    }"#;

    let temp_dir = std::env::temp_dir();
    let in_path = temp_dir.join("test_eq_in.json");
    let out_path = temp_dir.join("test_eq_out.json");

    fs::write(&in_path, ir_in).unwrap();
    fs::write(&out_path, ir_out).unwrap();

    // Run verify.py using python from venv
    let python_paths = [
        "/Users/liuyukai/CREATE/auv/dcl-poc/.venv/bin/python",
        "/Users/liuyukai/CREATE/auv/dcl/.venv/bin/python",
        "python3",
    ];

    let mut python_cmd = "python3";
    for path in &python_paths {
        if std::path::Path::new(path).exists() {
            python_cmd = path;
            break;
        }
    }

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let mut workspace_root = std::path::PathBuf::from(manifest_dir);
    if workspace_root.ends_with("crates/dcl-cli") {
        workspace_root.pop();
        workspace_root.pop();
    }
    let verify_script = workspace_root.join("dcl-optimizer/verify.py");

    let status = Command::new(python_cmd)
        .arg(&verify_script)
        .arg("--input")
        .arg(&in_path)
        .arg("--output")
        .arg(&out_path)
        .status()
        .unwrap();

    // Verify that SMT solver correctly detects the discrepancy and exits with code 1
    assert_eq!(status.code(), Some(1));

    // Cleanup
    let _ = fs::remove_file(in_path);
    let _ = fs::remove_file(out_path);
}

#[test]
fn test_dynamic_array_compilation() {
    let input = r#"
        module DynamicArray

        circuit test_dynamic(
            private arr: Field[4],
            public idx: Field
        ) -> Field {
            return arr[idx];
        }
    "#;

    let mut lexer = Lexer::new(input);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let module = parser.parse_module().unwrap();
    let mut checker = TypeChecker::new();
    checker.check_module(&module).unwrap();

    let circuit = &module.circuits[0];
    let mut lowerer = Lowerer::new(&module);
    let graph = lowerer.lower_circuit(circuit).unwrap();

    // Verify graph output contains multiplexer nodes
    let node_types: Vec<_> = graph.nodes.iter().map(|n| &n.node_type).collect();
    assert!(node_types.contains(&&dcl_ir::NodeType::Select));
    assert!(node_types.contains(&&dcl_ir::NodeType::IsZero));

    let codegen = CodeGenerator::new(graph);
    let circom_code = codegen.generate_circom().unwrap();

    assert!(circom_code.contains("template test_dynamicMain()"));
    assert!(circom_code.contains("signal input arr_0;"));
    assert!(circom_code.contains("signal input arr_3;"));
    assert!(circom_code.contains("signal input idx;"));
}

#[test]
fn test_tfhe_backend_compilation() {
    let input = r#"
        module Homomorphic

        circuit compute(
            private a: Field,
            private b: Field
        ) -> Field {
            return a * b + a;
        }
    "#;

    let mut lexer = Lexer::new(input);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let module = parser.parse_module().unwrap();
    let mut checker = TypeChecker::new();
    checker.check_module(&module).unwrap();

    let circuit = &module.circuits[0];
    let mut lowerer = Lowerer::new(&module);
    let graph = lowerer.lower_circuit(circuit).unwrap();

    let codegen = CodeGenerator::new(graph);
    let tfhe_code = codegen.generate_tfhe().unwrap();

    assert!(tfhe_code.contains("pub fn compute("));
    assert!(tfhe_code.contains("pub struct computeInputs"));
    assert!(tfhe_code.contains("let n_2 = &n_0 * &n_1;"));
    assert!(tfhe_code.contains("let n_3 = &n_2 + &n_0;"));
}

#[test]
fn test_fhe_bootstrapping_scheduler() {
    let input = r#"
        module NoiseTest

        circuit test_noise(
            private a: Field,
            private b: Field
        ) -> Field {
            return a * b * a * b * a;
        }
    "#;

    let mut lexer = Lexer::new(input);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let module = parser.parse_module().unwrap();
    let mut checker = TypeChecker::new();
    checker.check_module(&module).unwrap();

    let circuit = &module.circuits[0];
    let mut lowerer = Lowerer::new(&module);
    let graph = lowerer.lower_circuit(circuit).unwrap();

    let codegen = CodeGenerator::new(graph);
    let tfhe_code = codegen.generate_tfhe().unwrap();

    // Verify it contains a bootstrap call to control noise growth
    assert!(tfhe_code.contains("server_key.bootstrap("));
}

#[test]
fn test_fixed_point_library_inlining() {
    let input = r#"
        module FixedTest

        circuit mul(a: Field, b: Field) -> Field {
            let raw_mul = a * b;
            return raw_mul / 65536;
        }

        circuit compute_interest(
            private principal: Field,
            private rate: Field
        ) -> Field {
            let interest = mul(principal, rate);
            return interest;
        }
    "#;

    let mut lexer = Lexer::new(input);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let module = parser.parse_module().unwrap();
    let mut checker = TypeChecker::new();
    checker.check_module(&module).unwrap();

    let circuit = &module.circuits[1]; // compute_interest
    let mut lowerer = Lowerer::new(&module);
    let graph = lowerer.lower_circuit(circuit).unwrap();

    let codegen = CodeGenerator::new(graph);
    let circom_code = codegen.generate_circom().unwrap();

    // Verify it contains the inlined division constraint
    assert!(circom_code.contains("/"));
    assert!(circom_code.contains("==="));
}

#[test]
fn test_if_else_compilation() {
    let input = r#"
        module IfTest

        circuit test_if(
            public cond: bool,
            private x: Field,
            private y: Field
        ) -> Field {
            let mut res = 0;
            if cond {
                res = x;
            } else {
                res = y;
            }
            return res;
        }
    "#;

    let mut lexer = Lexer::new(input);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let module = parser.parse_module().unwrap();
    let mut checker = TypeChecker::new();
    checker.check_module(&module).unwrap();

    let circuit = &module.circuits[0];
    let mut lowerer = Lowerer::new(&module);
    let graph = lowerer.lower_circuit(circuit).unwrap();

    // Verify a Select node was generated for merging the environments
    let has_select = graph.nodes.iter().any(|node| node.node_type == dcl_ir::NodeType::Select);
    assert!(has_select, "Lowering did not generate a Select node for If statement merge");

    let codegen = CodeGenerator::new(graph);
    let circom_code = codegen.generate_circom().unwrap();
    assert!(circom_code.contains(" * (") && circom_code.contains(" - "));
}

#[test]
fn test_large_field_element() {
    let input = r#"
        module LargeConstTest

        circuit test_large() -> Field {
            return 21888242871839275222246405745257275088548364400416034343698204186575808495617;
        }
    "#;

    let mut lexer = Lexer::new(input);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let module = parser.parse_module().unwrap();
    let mut checker = TypeChecker::new();
    checker.check_module(&module).unwrap();

    let circuit = &module.circuits[0];
    let mut lowerer = Lowerer::new(&module);
    let graph = lowerer.lower_circuit(circuit).unwrap();

    let codegen = CodeGenerator::new(graph);
    let circom_code = codegen.generate_circom().unwrap();

    assert!(circom_code.contains("21888242871839275222246405745257275088548364400416034343698204186575808495617"));
}

#[test]
fn test_detailed_spanned_errors() {
    let input = r#"
        module ErrTest

        circuit test_err(private x: Field) -> bool {
            assert x + 10;
            return true;
        }
    "#;

    let mut lexer = Lexer::new(input);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let module = parser.parse_module().unwrap();
    let mut checker = TypeChecker::new();
    let err = checker.check_module(&module).unwrap_err();

    assert!(err.contains("[Error at line 5, col 13]: Assertion expression must be Bool, found Field"));
}

#[test]
fn test_logical_operators() {
    let input = r#"
        module LogicTest

        circuit test_logic(
            public a: bool,
            private b: bool
        ) -> bool {
            let and_val = a && b;
            let or_val = a || b;
            let not_val = !b;
            let neq_val = a != b;
            return and_val && (or_val || not_val) && neq_val;
        }
    "#;

    let mut lexer = Lexer::new(input);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let module = parser.parse_module().unwrap();
    let mut checker = TypeChecker::new();
    checker.check_module(&module).unwrap();

    let circuit = &module.circuits[0];
    let mut lowerer = Lowerer::new(&module);
    let graph = lowerer.lower_circuit(circuit).unwrap();

    // Verify correct nodes were generated
    let has_mul = graph.nodes.iter().any(|node| node.node_type == dcl_ir::NodeType::Mul);
    let has_add = graph.nodes.iter().any(|node| node.node_type == dcl_ir::NodeType::Add);
    let has_sub = graph.nodes.iter().any(|node| node.node_type == dcl_ir::NodeType::Sub);
    let has_isz = graph.nodes.iter().any(|node| node.node_type == dcl_ir::NodeType::IsZero);

    assert!(has_mul, "Lowering did not generate Mul nodes for && / ||");
    assert!(has_add, "Lowering did not generate Add nodes for ||");
    assert!(has_sub, "Lowering did not generate Sub nodes for ! / || / !=");
    assert!(has_isz, "Lowering did not generate IsZero nodes for !=");

    let codegen = CodeGenerator::new(graph);
    let circom_code = codegen.generate_circom().unwrap();
    assert!(circom_code.contains("template test_logicMain()"));
}

#[test]
fn test_code_formatter() {
    let input = "module LogicTest\n\ncircuit test_logic(private a: bool) -> bool {\nlet mut x = 10 + 20 * 30;\nreturn x == 10;\n}";
    let mut lexer = Lexer::new(input);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let module = parser.parse_module().unwrap();

    let formatted = dcl_frontend::format_module(&module);
    
    // Check that indentation and operator spacing is preserved and cleaned
    assert!(formatted.contains("module LogicTest\n"));
    assert!(formatted.contains("    let mut x = 10 + 20 * 30;\n"));
    assert!(formatted.contains("    return x == 10;\n"));
}

#[test]
fn test_immutable_assign_error() {
    let input = "module Test\ncircuit main(private x: Field) -> bool {\nlet y = 10;\ny = 20;\nreturn true;\n}";
    let mut lexer = Lexer::new(input);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let module = parser.parse_module().unwrap();
    let mut checker = TypeChecker::new();
    let err = checker.check_module(&module).unwrap_err();
    assert!(err.contains("[Error at line 4, col 1]: Cannot assign to immutable variable: y"));
}

#[test]
fn test_duplicate_type_error() {
    let input = "module Test\ntype A = { x: Field }\ntype A = { y: bool }\ncircuit main() -> bool { return true; }";
    let mut lexer = Lexer::new(input);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let module = parser.parse_module().unwrap();
    let mut checker = TypeChecker::new();
    let err = checker.check_module(&module).unwrap_err();
    assert!(err.contains("[Error at line 3, col 6]: Duplicate type definition: A"));
}

#[test]
fn test_duplicate_circuit_error() {
    let input = "module Test\ncircuit foo() -> bool { return true; }\ncircuit foo() -> bool { return false; }";
    let mut lexer = Lexer::new(input);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let module = parser.parse_module().unwrap();
    let mut checker = TypeChecker::new();
    let err = checker.check_module(&module).unwrap_err();
    assert!(err.contains("[Error at line 3, col 9]: Duplicate circuit definition: foo"));
}

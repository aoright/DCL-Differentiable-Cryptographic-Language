//! DCIR (Differentiable Cryptographic Intermediate Representation) module.
//!
//! This crate defines the computation graph IR used between the DCL frontend and
//! backend code generators. It includes:
//! - A DAG-based [`Graph`] of typed [`Node`]s with strategy annotations
//! - The [`Lowerer`] that converts AST to DCIR
//! - Information flow analysis for detecting secret-to-public leaks
//! - Constant folding and dead code elimination optimization passes
//! - Under-constrained signal detection

use serde::{Serialize, Deserialize};
use std::collections::{HashMap, HashSet};
use dcl_frontend::ast::{self, Module, Circuit, Stmt, Expr, BinOp, UnOp, Type, Visibility as ASTVisibility};

/// Types of computation nodes in the DCIR graph.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum NodeType {
    Const,
    Input,
    Add,
    Sub,
    Mul,
    Div,
    Select,
    #[serde(rename = "assert_eq")]
    AssertEq,
    #[serde(rename = "assert_bool")]
    AssertBool,
    #[serde(rename = "range_check")]
    RangeCheck,
    Poseidon,
    #[serde(rename = "is_zero")]
    IsZero,
}

/// Visibility classification for ZKP witness inputs.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    Public,
    Private,
}

/// An implementation strategy for a node, with associated cost metrics.
///
/// The optimizer selects among multiple strategies using Gumbel-Softmax
/// continuous relaxation.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Strategy {
    pub name: String,
    pub constraint_cost: f64,
    pub depth_cost: f64,
    pub noise_cost: f64,
}

/// A single node in the DCIR computation graph.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Node {
    pub id: usize,
    pub node_type: NodeType,
    pub inputs: Vec<usize>,
    pub strategies: Vec<Strategy>,
    pub alpha: Option<Vec<f64>>,
    pub value: Option<String>,
    pub bits: Option<usize>,
    pub visibility: Option<Visibility>,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
}

/// A directed acyclic computation graph representing a circuit.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Graph {
    pub name: String,
    pub nodes: Vec<Node>,
    pub outputs: Vec<usize>,
}

/// Information flow classification for taint analysis.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Secrecy {
    /// Value is publicly known (safe to expose).
    Public,
    /// Value contains or derives from private secrets.
    Secret,
}

/// Security analysis configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityLevel {
    /// Information flow violations produce warnings only.
    Warn,
    /// Information flow violations produce errors and abort compilation.
    Error,
}

impl Graph {
    /// Perform information flow analysis to detect private-to-public leaks.
    ///
    /// Traverses the computation graph, tagging each node as `Public` or `Secret`
    /// based on its inputs. Alerts if any secret data flows directly to an output
    /// without passing through a one-way function (e.g., Poseidon hash).
    pub fn check_information_flow(&self) -> Vec<String> {
        self.check_information_flow_with_level(SecurityLevel::Warn)
    }

    /// Information flow analysis with configurable severity level.
    pub fn check_information_flow_with_level(&self, level: SecurityLevel) -> Vec<String> {
        let mut diagnostics = Vec::new();
        let mut secrecy: HashMap<usize, Secrecy> = HashMap::new();
        let node_map: HashMap<usize, &Node> = self.nodes.iter().map(|n| (n.id, n)).collect();

        // 1. Traverse nodes and compute secrecy status
        for node in &self.nodes {
            let node_secrecy = match node.node_type {
                NodeType::Input => {
                    if node.visibility == Some(Visibility::Private) {
                        Secrecy::Secret
                    } else {
                        Secrecy::Public
                    }
                }
                NodeType::Const => Secrecy::Public,
                NodeType::Poseidon => Secrecy::Public, // poseidon is a secure one-way hash
                _ => {
                    let mut is_secret = false;
                    for &inp in &node.inputs {
                        if secrecy.get(&inp) == Some(&Secrecy::Secret) {
                            is_secret = true;
                            break;
                        }
                    }
                    if is_secret { Secrecy::Secret } else { Secrecy::Public }
                }
            };
            secrecy.insert(node.id, node_secrecy);
        }

        // 2. Check outputs for leaks
        let mut leaking_inputs = HashSet::new();
        for &out_id in &self.outputs {
            if secrecy.get(&out_id) == Some(&Secrecy::Secret) {
                self.trace_secret_inputs(out_id, &secrecy, &node_map, &mut leaking_inputs);
            }
        }

        if !leaking_inputs.is_empty() {
            let mut inputs_vec: Vec<String> = leaking_inputs.into_iter().collect();
            inputs_vec.sort();
            let prefix = match level {
                SecurityLevel::Warn => "⚠️  [Security Warning]",
                SecurityLevel::Error => "❌ [Security Error]",
            };
            let msg = format!(
                "{}: Private secret from input(s) '{}' leaks directly to public output in circuit '{}'. Consider passing secrets through a one-way hash function (like poseidon) before exporting.",
                prefix, inputs_vec.join(", "), self.name
            );
            diagnostics.push(msg.clone());
            eprintln!("{}", msg);
        }

        // 3. Check assert conditions for information leakage
        for node in &self.nodes {
            if node.node_type == NodeType::AssertEq {
                for &inp in &node.inputs {
                    if secrecy.get(&inp) == Some(&Secrecy::Secret) {
                        let msg = format!(
                            "⚠️  [Security Warning]: Assert condition in circuit '{}' references secret data (node '{}'), which may leak information through constraint failure timing.",
                            self.name, node.label
                        );
                        diagnostics.push(msg.clone());
                        eprintln!("{}", msg);
                        break;
                    }
                }
            }
        }

        // 4. Under-constrained signal detection
        let referenced: HashSet<usize> = self.nodes.iter()
            .flat_map(|n| n.inputs.iter().copied())
            .chain(self.outputs.iter().copied())
            .collect();

        for node in &self.nodes {
            if node.node_type == NodeType::Input && !referenced.contains(&node.id) {
                let msg = format!(
                    "⚠️  [Warning]: Input signal '{}' in circuit '{}' is never referenced — possible under-constrained signal.",
                    node.label, self.name
                );
                diagnostics.push(msg.clone());
                eprintln!("{}", msg);
            }
        }

        diagnostics
    }

    fn trace_secret_inputs(&self, node_id: usize, secrecy: &HashMap<usize, Secrecy>, node_map: &HashMap<usize, &Node>, leaking: &mut HashSet<String>) {
        let node = match node_map.get(&node_id) {
            Some(n) => *n,
            None => return,
        };

        if node.node_type == NodeType::Input {
            if node.visibility == Some(Visibility::Private) {
                leaking.insert(node.label.clone());
            }
            return;
        }

        for &inp in &node.inputs {
            if secrecy.get(&inp) == Some(&Secrecy::Secret) {
                self.trace_secret_inputs(inp, secrecy, node_map, leaking);
            }
        }
    }

    /// Constant folding: replace sub-expressions with compile-time known values.
    ///
    /// Evaluates arithmetic on constant nodes: `Const(2) + Const(3)` → `Const(5)`.
    pub fn constant_fold(&mut self) {
        let mut changed = true;
        while changed {
            changed = false;
            let snapshot: Vec<Node> = self.nodes.clone();
            let val_map: HashMap<usize, &str> = snapshot.iter()
                .filter(|n| n.node_type == NodeType::Const)
                .filter_map(|n| n.value.as_deref().map(|v| (n.id, v)))
                .collect();

            for node in self.nodes.iter_mut() {
                if node.node_type == NodeType::Const { continue; }
                if node.inputs.len() == 2 {
                    let a = val_map.get(&node.inputs[0]).and_then(|s| s.parse::<i128>().ok());
                    let b = val_map.get(&node.inputs[1]).and_then(|s| s.parse::<i128>().ok());
                    if let (Some(va), Some(vb)) = (a, b) {
                        let result = match node.node_type {
                            NodeType::Add => Some(va + vb),
                            NodeType::Sub => Some(va - vb),
                            NodeType::Mul => Some(va * vb),
                            _ => None,
                        };
                        if let Some(r) = result {
                            node.node_type = NodeType::Const;
                            node.inputs.clear();
                            node.value = Some(r.to_string());
                            node.label = format!("const_folded_{}", r);
                            changed = true;
                        }
                    }
                }
            }
        }
    }

    /// Dead code elimination: remove nodes not reachable from outputs or assertions.
    pub fn dead_code_eliminate(&mut self) {
        let mut live: HashSet<usize> = HashSet::new();
        let mut worklist: Vec<usize> = self.outputs.clone();

        // All AssertEq/AssertBool nodes are live roots too
        for node in &self.nodes {
            if matches!(node.node_type, NodeType::AssertEq | NodeType::AssertBool) {
                worklist.push(node.id);
            }
        }

        // BFS from live roots
        while let Some(id) = worklist.pop() {
            if !live.insert(id) { continue; }
            if let Some(node) = self.nodes.iter().find(|n| n.id == id) {
                for &inp in &node.inputs {
                    worklist.push(inp);
                }
            }
        }

        self.nodes.retain(|n| live.contains(&n.id));
    }
}

/// Lowers a DCL AST circuit into a DCIR computation graph.
///
/// Handles struct flattening, loop unrolling, conditional branch merging (via
/// Select MUX nodes), function inlining, and strategy annotation.
pub struct Lowerer {
    nodes: Vec<Node>,
    next_id: usize,
    env: HashMap<String, usize>, // maps variable name to node ID
    struct_defs: HashMap<String, ast::StructDef>,
    circuits: HashMap<String, ast::Circuit>,
    condition_stack: Vec<usize>,
    current_line: Option<usize>,
}

impl Lowerer {
    pub fn new(module: &Module) -> Self {
        let mut struct_defs = HashMap::new();
        for s in &module.types {
            struct_defs.insert(s.name.clone(), s.clone());
        }
        let mut circuits = HashMap::new();
        for c in &module.circuits {
            circuits.insert(c.name.clone(), c.clone());
        }
        Self {
            nodes: Vec::new(),
            next_id: 0,
            env: HashMap::new(),
            struct_defs,
            circuits,
            condition_stack: Vec::new(),
            current_line: None,
        }
    }

    fn alloc_id(&mut self) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn add_node(&mut self, node_type: NodeType, inputs: Vec<usize>, label: String) -> usize {
        let id = self.alloc_id();
        self.nodes.push(Node {
            id,
            node_type,
            inputs,
            strategies: Vec::new(),
            alpha: None,
            value: None,
            bits: None,
            visibility: None,
            label,
            line: self.current_line,
        });
        id
    }

    fn add_const_node(&mut self, value: String, label: String) -> usize {
        let id = self.alloc_id();
        self.nodes.push(Node {
            id,
            node_type: NodeType::Const,
            inputs: Vec::new(),
            strategies: Vec::new(),
            alpha: None,
            value: Some(value),
            bits: None,
            visibility: None,
            label,
            line: self.current_line,
        });
        id
    }

    pub fn lower_circuit(&mut self, circuit: &Circuit) -> Result<Graph, String> {
        self.nodes.clear();
        self.next_id = 0;
        self.env.clear();
        self.condition_stack.clear();
        self.current_line = None;

        // 1. Lower parameters. Since struct arguments are flattened, we register each leaf field as an input.
        for param in &circuit.params {
            let vis = match param.visibility {
                ASTVisibility::Public => Visibility::Public,
                _ => Visibility::Private,
            };
            self.lower_parameter(&param.name, &param.ty, vis)?;
        }

        // 2. Lower body statements
        let mut outputs = Vec::new();
        for stmt in &circuit.body {
            self.lower_statement(stmt, &mut outputs)?;
        }

        Ok(Graph {
            name: circuit.name.clone(),
            nodes: self.nodes.clone(),
            outputs,
        })
    }

    fn lower_statement(&mut self, stmt: &Stmt, outputs: &mut Vec<usize>) -> Result<(), String> {
        self.current_line = Some(stmt.span().line());
        match stmt {
            Stmt::Let(name, _is_mut, _, expr, _) => {
                let val_id = self.lower_expr(expr)?;
                self.env.insert(name.clone(), val_id);
            }
            Stmt::Assert(expr, _) => {
                let expr_id = self.lower_expr(expr)?;
                if self.condition_stack.is_empty() {
                    let true_id = self.add_const_node("1".to_string(), "const_true".to_string());
                    self.add_node(NodeType::AssertEq, vec![expr_id, true_id], format!("assert_eq_{}", expr_id));
                } else {
                    let mut p = self.condition_stack[0];
                    for i in 1..self.condition_stack.len() {
                        let c = self.condition_stack[i];
                        p = self.add_node(NodeType::Mul, vec![p, c], format!("path_cond_step_{}_{}", p, c));
                    }
                    let one_id = self.add_const_node("1".to_string(), "const_1".to_string());
                    let not_expr_id = self.add_node(NodeType::Sub, vec![one_id, expr_id], format!("not_assert_expr_{}", expr_id));
                    let assert_val_id = self.add_node(NodeType::Mul, vec![p, not_expr_id], format!("assert_cond_val_{}", expr_id));
                    let zero_id = self.add_const_node("0".to_string(), "const_0".to_string());
                    self.add_node(NodeType::AssertEq, vec![assert_val_id, zero_id], format!("assert_eq_zero_{}", expr_id));
                }
            }
            Stmt::Assign(lhs, rhs, _) => {
                let rhs_id = self.lower_expr(rhs)?;
                let path = self.resolve_path(lhs)?;
                self.env.insert(path, rhs_id);
            }
            Stmt::Return(expr, _) => {
                let ret_id = self.lower_expr(expr)?;
                outputs.push(ret_id);
            }
            Stmt::For(var_name, start_expr, end_expr, body, _) => {
                use num_traits::ToPrimitive;
                let start = self.eval_expr_to_const(start_expr)?
                    .to_usize()
                    .ok_or_else(|| "Loop start bound exceeds usize capacity".to_string())?;
                let end = self.eval_expr_to_const(end_expr)?
                    .to_usize()
                    .ok_or_else(|| "Loop end bound exceeds usize capacity".to_string())?;
                for i in start..end {
                    let i_id = self.alloc_id();
                    self.nodes.push(Node {
                        id: i_id,
                        node_type: NodeType::Const,
                        inputs: Vec::new(),
                        strategies: Vec::new(),
                        alpha: None,
                        value: Some(i.to_string()),
                        bits: None,
                        visibility: None,
                        label: format!("loop_idx_{}", i),
                        line: self.current_line,
                    });
                    self.env.insert(var_name.clone(), i_id);

                    for s in body {
                        self.lower_statement(s, outputs)?;
                    }
                }
                self.env.remove(var_name);
            }
            Stmt::If(cond, then_body, else_body, _) => {
                let cond_id = self.lower_expr(cond)?;
                let original_env = self.env.clone();

                // 1. Lower then branch
                self.condition_stack.push(cond_id);
                for s in then_body {
                    self.lower_statement(s, outputs)?;
                }
                self.condition_stack.pop();
                let then_env = self.env.clone();

                // 2. Lower else branch
                self.env = original_env.clone();
                if let Some(else_stmts) = else_body {
                    let one_id = self.add_const_node("1".to_string(), "const_1".to_string());
                    let not_cond_id = self.add_node(NodeType::Sub, vec![one_id, cond_id], "not_cond".to_string());
                    self.condition_stack.push(not_cond_id);
                    for s in else_stmts {
                        self.lower_statement(s, outputs)?;
                    }
                    self.condition_stack.pop();
                }
                let else_env = self.env.clone();

                // 3. Merge environment side effects
                let mut merged_env = original_env;
                let all_vars: std::collections::HashSet<&String> = then_env.keys().chain(else_env.keys()).collect();
                for var in all_vars {
                    let id_then = then_env.get(var).cloned();
                    let id_else = else_env.get(var).cloned();
                    match (id_then, id_else) {
                        (Some(t), Some(e)) if t != e => {
                            let select_id = self.alloc_id();
                            self.nodes.push(Node {
                                id: select_id,
                                node_type: NodeType::Select,
                                inputs: vec![cond_id, t, e],
                                strategies: Vec::new(),
                                alpha: None,
                                value: None,
                                bits: None,
                                visibility: None,
                                label: format!("if_merge_{}_{}", var, select_id),
                                line: self.current_line,
                            });
                            merged_env.insert((*var).clone(), select_id);
                        }
                        _ => {}
                    }
                }
                self.env = merged_env;
            }
            Stmt::ExprStmt(expr, _) => {
                self.lower_expr(expr)?;
            }
        }
        Ok(())
    }

    fn eval_expr_to_const(&self, expr: &Expr) -> Result<num_bigint::BigInt, String> {
        use num_traits::Zero;
        match expr {
            Expr::ConstField(v, _) => {
                v.parse::<num_bigint::BigInt>().map_err(|_| format!("Invalid constant field representation: {}", v))
            }
            Expr::Var(name, _) => {
                if let Some(&id) = self.env.get(name) {
                    let node = &self.nodes[id];
                    if node.node_type == NodeType::Const {
                        let val_str = node.value.as_deref().unwrap_or("0");
                        return val_str.parse::<num_bigint::BigInt>().map_err(|_| format!("Invalid constant value in node: {}", val_str));
                    }
                }
                Err(format!("Variable {} is not a compile-time constant", name))
            }
            Expr::Access(_, _, _) | Expr::Index(_, _, _) => {
                let path = self.resolve_path(expr)?;
                if let Some(&id) = self.env.get(&path) {
                    let node = &self.nodes[id];
                    if node.node_type == NodeType::Const {
                        let val_str = node.value.as_deref().unwrap_or("0");
                        return val_str.parse::<num_bigint::BigInt>().map_err(|_| format!("Invalid constant value in node: {}", val_str));
                    }
                }
                Err(format!("Path {} is not a compile-time constant", path))
            }
            Expr::Unary(UnOp::Not, inner, _) => {
                let val = self.eval_expr_to_const(inner)?;
                if val.is_zero() {
                    Ok(num_bigint::BigInt::from(1))
                } else {
                    Ok(num_bigint::BigInt::zero())
                }
            }
            Expr::Unary(UnOp::Neg, inner, _) => {
                let val = self.eval_expr_to_const(inner)?;
                Ok(-val)
            }
            Expr::Binary(op, lhs, rhs, _) => {
                let l = self.eval_expr_to_const(lhs)?;
                let r = self.eval_expr_to_const(rhs)?;
                match op {
                    BinOp::Add => Ok(l + r),
                    BinOp::Sub => Ok(l - r),
                    BinOp::Mul => Ok(l * r),
                    BinOp::Div => {
                        if r.is_zero() {
                            Err("Division by zero in constant evaluation".to_string())
                        } else {
                            Ok(l / r)
                        }
                    }
                    BinOp::And => {
                        if !l.is_zero() && !r.is_zero() {
                            Ok(num_bigint::BigInt::from(1))
                        } else {
                            Ok(num_bigint::BigInt::zero())
                        }
                    }
                    BinOp::Or => {
                        if !l.is_zero() || !r.is_zero() {
                            Ok(num_bigint::BigInt::from(1))
                        } else {
                            Ok(num_bigint::BigInt::zero())
                        }
                    }
                    BinOp::Eq => {
                        if l == r {
                            Ok(num_bigint::BigInt::from(1))
                        } else {
                            Ok(num_bigint::BigInt::zero())
                        }
                    }
                    BinOp::NotEq => {
                        if l != r {
                            Ok(num_bigint::BigInt::from(1))
                        } else {
                            Ok(num_bigint::BigInt::zero())
                        }
                    }
                    _ => Err("Unsupported operation in constant evaluation".to_string()),
                }
            }
            _ => Err("Expression is not a compile-time constant".to_string()),
        }
    }

    fn resolve_path(&self, expr: &Expr) -> Result<String, String> {
        match expr {
            Expr::Var(name, _) => Ok(name.clone()),
            Expr::Access(base, field_name, _) => {
                let base_path = self.resolve_path(base)?;
                Ok(format!("{}_{}", base_path, field_name))
            }
            Expr::Index(base, index, _) => {
                let base_path = self.resolve_path(base)?;
                use num_traits::ToPrimitive;
                let idx = self.eval_expr_to_const(index)?
                    .to_usize()
                    .ok_or_else(|| "Array index exceeds usize capacity".to_string())?;
                Ok(format!("{}_{}", base_path, idx))
            }
            _ => Err("Invalid path expression".to_string()),
        }
    }

    fn lower_parameter(&mut self, name: &str, ty: &Type, vis: Visibility) -> Result<(), String> {
        match ty {
            Type::Field | Type::Bool => {
                let id = self.alloc_id();
                self.nodes.push(Node {
                    id,
                    node_type: NodeType::Input,
                    inputs: Vec::new(),
                    strategies: Vec::new(),
                    alpha: None,
                    value: None,
                    bits: None,
                    visibility: Some(vis),
                    label: name.to_string(),
                    line: self.current_line,
                });
                self.env.insert(name.to_string(), id);
            }
            Type::Struct(sname) => {
                let def = self.struct_defs.get(sname).ok_or_else(|| format!("Unknown struct: {}", sname))?.clone();
                for (fname, fty) in &def.fields {
                    let flattened_name = format!("{}_{}", name, fname);
                    self.lower_parameter(&flattened_name, fty, vis.clone())?;
                }
            }
            Type::Array(inner, size) => {
                for i in 0..*size {
                    let flattened_name = format!("{}_{}", name, i);
                    self.lower_parameter(&flattened_name, inner, vis.clone())?;
                }
            }
        }
        Ok(())
    }

    fn lower_expr(&mut self, expr: &Expr) -> Result<usize, String> {
        self.current_line = Some(expr.span().line());
        match expr {
            Expr::Var(name, _) => {
                // If it's a direct variable name in the environment
                if let Some(&id) = self.env.get(name) {
                    Ok(id)
                } else {
                    Err(format!("Undefined variable in expression lowering: {}", name))
                }
            }
            Expr::ConstField(val, _) => {
                let id = self.alloc_id();
                self.nodes.push(Node {
                    id,
                    node_type: NodeType::Const,
                    inputs: Vec::new(),
                    strategies: Vec::new(),
                    alpha: None,
                    value: Some(val.clone()),
                    bits: None,
                    visibility: None,
                    label: format!("const_{}", val),
                    line: self.current_line,
                });
                Ok(id)
            }
            Expr::ConstBool(val, _) => {
                let id = self.alloc_id();
                self.nodes.push(Node {
                    id,
                    node_type: NodeType::Const,
                    inputs: Vec::new(),
                    strategies: Vec::new(),
                    alpha: None,
                    value: Some(if *val { "1".to_string() } else { "0".to_string() }),
                    bits: None,
                    visibility: None,
                    label: format!("const_bool_{}", val),
                    line: self.current_line,
                });
                Ok(id)
            }
            Expr::Unary(UnOp::Not, inner, _) => {
                let val_id = self.lower_expr(inner)?;
                let one_id = self.add_const_node("1".to_string(), "const_1".to_string());
                let not_id = self.add_node(NodeType::Sub, vec![one_id, val_id], format!("logical_not_{}", val_id));
                Ok(not_id)
            }
            Expr::Unary(UnOp::Neg, inner, _) => {
                let val_id = self.lower_expr(inner)?;
                let zero_id = self.add_const_node("0".to_string(), "const_0".to_string());
                let neg_id = self.add_node(NodeType::Sub, vec![zero_id, val_id], format!("neg_{}", val_id));
                Ok(neg_id)
            }
            Expr::Access(_, _, _) => {
                let path = self.resolve_path(expr)?;
                if let Some(&id) = self.env.get(&path) {
                    Ok(id)
                } else {
                    Err(format!("Could not resolve path: {}", path))
                }
            }
            Expr::Index(base, index, _) => {
                // Try static resolution first
                if let Ok(path) = self.resolve_path(expr) {
                    if let Some(&id) = self.env.get(&path) {
                        return Ok(id);
                    }
                }

                // If static resolution fails or is not in environment, build a dynamic multiplexer tree
                let idx_id = self.lower_expr(index)?;
                let base_path = self.resolve_path(base)?;

                // Find all elements of this array in env (e.g. base_path_0, base_path_1, ...)
                let mut elements = Vec::new();
                let mut k = 0;
                while let Some(&el_id) = self.env.get(&format!("{}_{}", base_path, k)) {
                    elements.push(el_id);
                    k += 1;
                }

                if elements.is_empty() {
                    return Err(format!("Base array variable not found or is empty: {}", base_path));
                }

                // Generate O(log N) binary MUX selection tree
                self.build_binary_mux(&elements, idx_id, 0)
            }
            Expr::Binary(op, lhs, rhs, _) => {
                let l_id = self.lower_expr(lhs)?;
                let r_id = self.lower_expr(rhs)?;
                match op {
                    BinOp::Add => {
                        let id = self.add_node(NodeType::Add, vec![l_id, r_id], format!("add_{}_{}", l_id, r_id));
                        Ok(id)
                    }
                    BinOp::Sub => {
                        let id = self.add_node(NodeType::Sub, vec![l_id, r_id], format!("sub_{}_{}", l_id, r_id));
                        Ok(id)
                    }
                    BinOp::Mul => {
                        let id = self.add_node(NodeType::Mul, vec![l_id, r_id], format!("mul_{}_{}", l_id, r_id));
                        Ok(id)
                    }
                    BinOp::Div => {
                        let id = self.add_node(NodeType::Div, vec![l_id, r_id], format!("div_{}_{}", l_id, r_id));
                        Ok(id)
                    }
                    BinOp::Eq => {
                        // a == b is IsZero(a - b)
                        let sub_id = self.add_node(NodeType::Sub, vec![l_id, r_id], format!("eq_diff_{}", l_id));
                        let isz_id = self.add_node(NodeType::IsZero, vec![sub_id], format!("eq_isz_{}", sub_id));
                        Ok(isz_id)
                    }
                    BinOp::NotEq => {
                        // a != b is 1 - IsZero(a - b)
                        let sub_id = self.add_node(NodeType::Sub, vec![l_id, r_id], format!("neq_diff_{}", l_id));
                        let isz_id = self.add_node(NodeType::IsZero, vec![sub_id], format!("neq_isz_{}", sub_id));
                        let one_id = self.alloc_id();
                        self.nodes.push(Node {
                            id: one_id,
                            node_type: NodeType::Const,
                            inputs: Vec::new(),
                            strategies: Vec::new(),
                            alpha: None,
                            value: Some("1".to_string()),
                            bits: None,
                            visibility: None,
                            label: "const_1".to_string(),
                            line: self.current_line,
                        });
                        let neq_id = self.add_node(NodeType::Sub, vec![one_id, isz_id], format!("logical_neq_{}", l_id));
                        Ok(neq_id)
                    }
                    BinOp::And => {
                        let id = self.add_node(NodeType::Mul, vec![l_id, r_id], format!("logical_and_{}_{}", l_id, r_id));
                        Ok(id)
                    }
                    BinOp::Or => {
                        // a || b is (a + b) - (a * b)
                        let sum_id = self.add_node(NodeType::Add, vec![l_id, r_id], format!("logical_or_sum_{}", l_id));
                        let prod_id = self.add_node(NodeType::Mul, vec![l_id, r_id], format!("logical_or_prod_{}", l_id));
                        let or_id = self.add_node(NodeType::Sub, vec![sum_id, prod_id], format!("logical_or_{}", l_id));
                        Ok(or_id)
                    }
                    BinOp::Gte => {
                        let bits = self.estimate_bitwidth(l_id).max(self.estimate_bitwidth(r_id)).max(8);
                        let sub_id = self.add_node(NodeType::Sub, vec![l_id, r_id], format!("cmp_diff_gte_{}", l_id));
                        let rc_id = self.alloc_id();
                        let strats = range_check_strategies(bits);
                        self.nodes.push(Node {
                            id: rc_id,
                            node_type: NodeType::RangeCheck,
                            inputs: vec![sub_id],
                            strategies: strats,
                            alpha: Some(vec![0.0; 3]),
                            value: None,
                            bits: Some(bits),
                            visibility: None,
                            label: format!("cmp_range_gte_{}", rc_id),
                            line: self.current_line,
                        });
                        Ok(rc_id)
                    }
                    BinOp::Gt => {
                        let bits = self.estimate_bitwidth(l_id).max(self.estimate_bitwidth(r_id)).max(8);
                        let sub_id = self.add_node(NodeType::Sub, vec![l_id, r_id], format!("cmp_diff_gt_{}", l_id));
                        let one_id = self.alloc_id();
                        self.nodes.push(Node {
                            id: one_id,
                            node_type: NodeType::Const,
                            inputs: Vec::new(),
                            strategies: Vec::new(),
                            alpha: None,
                            value: Some("1".to_string()),
                            bits: None,
                            visibility: None,
                            label: "const_1".to_string(),
                            line: self.current_line,
                        });
                        let diff_minus_one_id = self.add_node(NodeType::Sub, vec![sub_id, one_id], format!("cmp_diff_gt_minus_1_{}", l_id));
                        let rc_id = self.alloc_id();
                        let strats = range_check_strategies(bits);
                        self.nodes.push(Node {
                            id: rc_id,
                            node_type: NodeType::RangeCheck,
                            inputs: vec![diff_minus_one_id],
                            strategies: strats,
                            alpha: Some(vec![0.0; 3]),
                            value: None,
                            bits: Some(bits),
                            visibility: None,
                            label: format!("cmp_range_gt_{}", rc_id),
                            line: self.current_line,
                        });
                        Ok(rc_id)
                    }
                    BinOp::Lte => {
                        let bits = self.estimate_bitwidth(l_id).max(self.estimate_bitwidth(r_id)).max(8);
                        let sub_id = self.add_node(NodeType::Sub, vec![r_id, l_id], format!("cmp_diff_lte_{}", r_id));
                        let rc_id = self.alloc_id();
                        let strats = range_check_strategies(bits);
                        self.nodes.push(Node {
                            id: rc_id,
                            node_type: NodeType::RangeCheck,
                            inputs: vec![sub_id],
                            strategies: strats,
                            alpha: Some(vec![0.0; 3]),
                            value: None,
                            bits: Some(bits),
                            visibility: None,
                            label: format!("cmp_range_lte_{}", rc_id),
                            line: self.current_line,
                        });
                        Ok(rc_id)
                    }
                    BinOp::Lt => {
                        let bits = self.estimate_bitwidth(l_id).max(self.estimate_bitwidth(r_id)).max(8);
                        let sub_id = self.add_node(NodeType::Sub, vec![r_id, l_id], format!("cmp_diff_lt_{}", r_id));
                        let one_id = self.alloc_id();
                        self.nodes.push(Node {
                            id: one_id,
                            node_type: NodeType::Const,
                            inputs: Vec::new(),
                            strategies: Vec::new(),
                            alpha: None,
                            value: Some("1".to_string()),
                            bits: None,
                            visibility: None,
                            label: "const_1".to_string(),
                            line: self.current_line,
                        });
                        let diff_minus_one_id = self.add_node(NodeType::Sub, vec![sub_id, one_id], format!("cmp_diff_lt_minus_1_{}", r_id));
                        let rc_id = self.alloc_id();
                        let strats = range_check_strategies(bits);
                        self.nodes.push(Node {
                            id: rc_id,
                            node_type: NodeType::RangeCheck,
                            inputs: vec![diff_minus_one_id],
                            strategies: strats,
                            alpha: Some(vec![0.0; 3]),
                            value: None,
                            bits: Some(bits),
                            visibility: None,
                            label: format!("cmp_range_lt_{}", rc_id),
                            line: self.current_line,
                        });
                        Ok(rc_id)
                    }
                }
            }
            Expr::Call(name, args, _) => {
                let mut arg_ids = Vec::new();
                for arg in args {
                    arg_ids.push(self.lower_expr(arg)?);
                }

                match name.as_str() {
                    "poseidon" | "crypto::poseidon" | "std::crypto::poseidon" => {
                        let id = self.alloc_id();
                        let strats = poseidon_strategies(arg_ids.len());
                        self.nodes.push(Node {
                            id,
                            node_type: NodeType::Poseidon,
                            inputs: arg_ids,
                            strategies: strats,
                            alpha: Some(vec![0.0; 3]),
                            value: None,
                            bits: None,
                            visibility: None,
                            label: format!("poseidon_{}", id),
                            line: self.current_line,
                        });
                        Ok(id)
                    }
                    "range_check" | "utils::range_check" | "std::utils::range_check" => {
                        // args[0] = value, args[1] = bits constant
                        let val_id = arg_ids[0];
                        let bits = match &args[1] {
                            Expr::ConstField(b, _) => b.parse::<usize>().map_err(|_| "range_check bits parameter must be a valid integer".to_string())?,
                            _ => return Err("range_check bits parameter must be a constant".to_string()),
                        };
                        let id = self.alloc_id();
                        let strats = range_check_strategies(bits);
                        self.nodes.push(Node {
                            id,
                            node_type: NodeType::RangeCheck,
                            inputs: vec![val_id],
                            strategies: strats,
                            alpha: Some(vec![0.0; 3]),
                            value: None,
                            bits: Some(bits),
                            visibility: None,
                            label: format!("range_{}bit_{}", bits, id),
                            line: self.current_line,
                        });
                        Ok(id)
                    }
                    other => {
                        if let Some(target_circuit) = self.circuits.get(other).cloned() {
                            let saved_env = self.env.clone();

                            // Bind actual arguments to formal parameter names in the local scope
                            self.env.clear();
                            for (i, param) in target_circuit.params.iter().enumerate() {
                                self.env.insert(param.name.clone(), arg_ids[i]);
                            }

                            // Recursively lower the target circuit's statements
                            let mut inlined_outputs = Vec::new();
                            for stmt in &target_circuit.body {
                                self.lower_statement(stmt, &mut inlined_outputs)?;
                            }

                            // Restore calling environment
                            self.env = saved_env;

                            if let Some(&ret_id) = inlined_outputs.first() {
                                Ok(ret_id)
                            } else {
                                Err(format!("Inlined circuit '{}' did not return a value", other))
                            }
                        } else {
                            Err(format!("Unsupported circuit function call in lowering: {}", other))
                        }
                    }
                }
            }
        }
    }

    fn estimate_bitwidth(&self, node_id: usize) -> usize {
        let node = match self.nodes.iter().find(|n| n.id == node_id) {
            Some(n) => n,
            None => return 64,
        };
        match node.node_type {
            NodeType::Const => {
                if let Some(ref val_str) = node.value {
                    if let Ok(val) = val_str.parse::<u128>() {
                        let bits = 128 - val.leading_zeros() as usize;
                        return bits.max(8);
                    }
                }
                64
            }
            NodeType::RangeCheck => node.bits.unwrap_or(64),
            NodeType::Add => {
                let a = self.estimate_bitwidth(node.inputs[0]);
                let b = self.estimate_bitwidth(node.inputs[1]);
                a.max(b).saturating_add(1).min(64)
            }
            NodeType::Sub => {
                let a = self.estimate_bitwidth(node.inputs[0]);
                let b = self.estimate_bitwidth(node.inputs[1]);
                a.max(b).min(64)
            }
            NodeType::Mul => {
                let a = self.estimate_bitwidth(node.inputs[0]);
                let b = self.estimate_bitwidth(node.inputs[1]);
                a.saturating_add(b).min(64)
            }
            _ => 64,
        }
    }

    fn build_binary_mux(&mut self, elements: &[usize], idx_id: usize, start: usize) -> Result<usize, String> {
        if elements.is_empty() {
            return Ok(self.add_const_node("0".to_string(), "mux_empty".to_string()));
        }
        if elements.len() == 1 {
            return Ok(elements[0]);
        }
        if elements.len() == 2 {
            let const_start_id = self.add_const_node(start.to_string(), format!("mux_const_{}", start));
            let diff_id = self.add_node(NodeType::Sub, vec![idx_id, const_start_id], format!("mux_diff_{}", start));
            let cond_id = self.add_node(NodeType::IsZero, vec![diff_id], format!("mux_cond_{}", start));
            let select_id = self.add_node(NodeType::Select, vec![cond_id, elements[0], elements[1]], format!("mux_select_leaf_{}", start));
            return Ok(select_id);
        }

        let mid_idx = elements.len() / 2;
        let mid_val = start + mid_idx;

        let const_mid_id = self.add_const_node(mid_val.to_string(), format!("mux_const_mid_{}", mid_val));
        let diff_id = self.add_node(NodeType::Sub, vec![idx_id, const_mid_id], format!("mux_diff_mid_{}", mid_val));
        
        let cond_id = self.alloc_id();
        let strats = range_check_strategies(64);
        self.nodes.push(Node {
            id: cond_id,
            node_type: NodeType::RangeCheck,
            inputs: vec![diff_id],
            strategies: strats,
            alpha: Some(vec![0.0; 3]),
            value: None,
            bits: Some(64),
            visibility: None,
            label: format!("mux_range_check_{}", cond_id),
            line: self.current_line,
        });

        let left_id = self.build_binary_mux(&elements[..mid_idx], idx_id, start)?;
        let right_id = self.build_binary_mux(&elements[mid_idx..], idx_id, mid_val)?;

        let select_id = self.add_node(NodeType::Select, vec![cond_id, right_id, left_id], format!("mux_select_node_{}", mid_val));
        Ok(select_id)
    }
}

/// Generate candidate strategies for a range check of `bits` width.
///
/// Returns three strategies: boolean decomposition, lookup table, and polynomial approximation.
fn range_check_strategies(bits: usize) -> Vec<Strategy> {
    vec![
        Strategy {
            name: "boolean_decomp".to_string(),
            constraint_cost: bits as f64,
            depth_cost: 1.0,
            noise_cost: bits as f64 * 0.5,
        },
        Strategy {
            name: "lookup_table".to_string(),
            constraint_cost: 1.0,
            depth_cost: (bits as f64 / 2.0).max(1.0),
            noise_cost: bits as f64 * 1.5,
        },
        Strategy {
            name: "polynomial_approx".to_string(),
            constraint_cost: (bits as f64 / 2.0).max(1.0),
            depth_cost: (bits as f64 / 2.0).max(1.0),
            noise_cost: bits as f64 * 2.0,
        },
    ]
}

fn poseidon_strategies(num_inputs: usize) -> Vec<Strategy> {
    let t = num_inputs + 1;
    let base_full = 8.0;
    let base_partial = 56.0f64.max(3.0 * t as f64);
    let full_round_cost = t as f64;
    let partial_round_cost = 1.0;

    let standard_cost = base_full * full_round_cost + base_partial * partial_round_cost;
    let standard_depth = base_full + base_partial;

    vec![
        Strategy {
            name: "standard".to_string(),
            constraint_cost: standard_cost,
            depth_cost: standard_depth,
            noise_cost: (base_full * full_round_cost + base_partial) * 0.8,
        },
        Strategy {
            name: "partial_optimized".to_string(),
            constraint_cost: base_full * full_round_cost + base_partial * 0.6,
            depth_cost: base_full + base_partial * 0.7,
            noise_cost: (base_full * full_round_cost + base_partial * 0.6) * 0.9,
        },
        Strategy {
            name: "lookup_assisted".to_string(),
            constraint_cost: base_full * 2.0 + base_partial * 0.3,
            depth_cost: base_full + base_partial * 0.5,
            noise_cost: (base_full * 3.0 + base_partial) * 1.2,
        },
    ]
}

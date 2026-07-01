use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use dcl_frontend::ast::{self, Module, Circuit, Stmt, Expr, BinOp, Type, Visibility as ASTVisibility};

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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    Public,
    Private,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Strategy {
    pub name: String,
    pub constraint_cost: f64,
    pub depth_cost: f64,
    pub noise_cost: f64,
}

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
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Graph {
    pub name: String,
    pub nodes: Vec<Node>,
    pub outputs: Vec<usize>,
}

pub struct Lowerer {
    nodes: Vec<Node>,
    next_id: usize,
    env: HashMap<String, usize>, // maps variable name to node ID
    struct_defs: HashMap<String, ast::StructDef>,
    circuits: HashMap<String, ast::Circuit>,
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
        });
        id
    }

    pub fn lower_circuit(&mut self, circuit: &Circuit) -> Result<Graph, String> {
        self.nodes.clear();
        self.next_id = 0;
        self.env.clear();

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
        match stmt {
            Stmt::Let(name, _is_mut, _, expr, _) => {
                let val_id = self.lower_expr(expr)?;
                self.env.insert(name.clone(), val_id);
            }
            Stmt::Assert(expr, _) => {
                let expr_id = self.lower_expr(expr)?;
                let true_id = self.alloc_id();
                self.nodes.push(Node {
                    id: true_id,
                    node_type: NodeType::Const,
                    inputs: Vec::new(),
                    strategies: Vec::new(),
                    alpha: None,
                    value: Some("1".to_string()),
                    bits: None,
                    visibility: None,
                    label: "const_true".to_string(),
                });
                self.add_node(NodeType::AssertEq, vec![expr_id, true_id], format!("assert_eq_{}", expr_id));
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
                for s in then_body {
                    self.lower_statement(s, outputs)?;
                }
                let then_env = self.env.clone();

                // 2. Lower else branch
                self.env = original_env.clone();
                if let Some(else_stmts) = else_body {
                    for s in else_stmts {
                        self.lower_statement(s, outputs)?;
                    }
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
                            });
                            merged_env.insert((*var).clone(), select_id);
                        }
                        _ => {}
                    }
                }
                self.env = merged_env;
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
            Expr::Unary(ast::UnOp::Not, inner, _) => {
                let val = self.eval_expr_to_const(inner)?;
                if val.is_zero() {
                    Ok(num_bigint::BigInt::from(1))
                } else {
                    Ok(num_bigint::BigInt::zero())
                }
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
                });
                Ok(id)
            }
            Expr::Unary(ast::UnOp::Not, inner, _) => {
                let val_id = self.lower_expr(inner)?;
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
                });
                let not_id = self.add_node(NodeType::Sub, vec![one_id, val_id], format!("logical_not_{}", val_id));
                Ok(not_id)
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

                // Generate MUX selection tree
                let mut current_val_id = self.add_const_node("0".to_string(), "mux_base_0".to_string());

                for (idx_val, &el_id) in elements.iter().enumerate().rev() {
                    let const_k_id = self.add_const_node(idx_val.to_string(), format!("mux_const_{}", idx_val));
                    let diff_id = self.add_node(NodeType::Sub, vec![idx_id, const_k_id], format!("mux_diff_{}_{}", idx_id, const_k_id));
                    let cond_id = self.add_node(NodeType::IsZero, vec![diff_id], format!("mux_cond_{}", diff_id));
                    
                    // Create Select node: select(cond_id, el_id, current_val_id)
                    current_val_id = self.add_node(NodeType::Select, vec![cond_id, el_id, current_val_id], format!("mux_select_{}", cond_id));
                }

                Ok(current_val_id)
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
                        // a >= b is range_check(a - b, 64)
                        let sub_id = self.add_node(NodeType::Sub, vec![l_id, r_id], format!("cmp_diff_gte_{}", l_id));
                        let rc_id = self.alloc_id();
                        let strats = range_check_strategies(64);
                        self.nodes.push(Node {
                            id: rc_id,
                            node_type: NodeType::RangeCheck,
                            inputs: vec![sub_id],
                            strategies: strats,
                            alpha: Some(vec![0.0; 3]),
                            value: None,
                            bits: Some(64),
                            visibility: None,
                            label: format!("cmp_range_gte_{}", rc_id),
                        });
                        Ok(rc_id)
                    }
                    BinOp::Gt => {
                        // a > b is range_check(a - b - 1, 64)
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
                        });
                        let diff_minus_one_id = self.add_node(NodeType::Sub, vec![sub_id, one_id], format!("cmp_diff_gt_minus_1_{}", l_id));
                        let rc_id = self.alloc_id();
                        let strats = range_check_strategies(64);
                        self.nodes.push(Node {
                            id: rc_id,
                            node_type: NodeType::RangeCheck,
                            inputs: vec![diff_minus_one_id],
                            strategies: strats,
                            alpha: Some(vec![0.0; 3]),
                            value: None,
                            bits: Some(64),
                            visibility: None,
                            label: format!("cmp_range_gt_{}", rc_id),
                        });
                        Ok(rc_id)
                    }
                    BinOp::Lte => {
                        // a <= b is range_check(b - a, 64)
                        let sub_id = self.add_node(NodeType::Sub, vec![r_id, l_id], format!("cmp_diff_lte_{}", r_id));
                        let rc_id = self.alloc_id();
                        let strats = range_check_strategies(64);
                        self.nodes.push(Node {
                            id: rc_id,
                            node_type: NodeType::RangeCheck,
                            inputs: vec![sub_id],
                            strategies: strats,
                            alpha: Some(vec![0.0; 3]),
                            value: None,
                            bits: Some(64),
                            visibility: None,
                            label: format!("cmp_range_lte_{}", rc_id),
                        });
                        Ok(rc_id)
                    }
                    BinOp::Lt => {
                        // a < b is range_check(b - a - 1, 64)
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
                        });
                        let diff_minus_one_id = self.add_node(NodeType::Sub, vec![sub_id, one_id], format!("cmp_diff_lt_minus_1_{}", r_id));
                        let rc_id = self.alloc_id();
                        let strats = range_check_strategies(64);
                        self.nodes.push(Node {
                            id: rc_id,
                            node_type: NodeType::RangeCheck,
                            inputs: vec![diff_minus_one_id],
                            strategies: strats,
                            alpha: Some(vec![0.0; 3]),
                            value: None,
                            bits: Some(64),
                            visibility: None,
                            label: format!("cmp_range_lt_{}", rc_id),
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
}

// Strategy helper functions (matching the Python cost model)
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

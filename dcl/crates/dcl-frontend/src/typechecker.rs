//! Static type checker for the DCL language.
//!
//! Validates type correctness across all circuits in a module, including:
//! - Expression type inference
//! - Assignment mutability checks
//! - Function call arity and argument type validation
//! - Recursive struct detection
//! - "Did you mean?" suggestions for undefined identifiers
//!
//! Uses error recovery to report multiple diagnostics per compilation run.

use crate::ast::*;
use std::collections::HashMap;

/// Type checker with error accumulation and recovery.
pub struct TypeChecker {
    structs: HashMap<String, StructDef>,
    circuits: HashMap<String, Circuit>,
    variables: HashMap<String, (Type, bool)>, // (type, is_mut)
    /// Accumulated type errors.
    pub errors: Vec<String>,
}

impl TypeChecker {
    /// Create a new, empty type checker.
    pub fn new() -> Self {
        Self {
            structs: HashMap::new(),
            circuits: HashMap::new(),
            variables: HashMap::new(),
            errors: Vec::new(),
        }
    }

    fn type_error(&mut self, span: Span, msg: &str) {
        let code = if msg.contains("Duplicate type") {
            "DCL-E101"
        } else if msg.contains("Duplicate circuit") {
            "DCL-E102"
        } else if msg.contains("missing return statement") {
            "DCL-E103"
        } else if msg.contains("Cannot assign to immutable") {
            "DCL-E104"
        } else if msg.contains("type mismatch") 
            || msg.contains("does not match") 
            || msg.contains("must be Bool") 
            || msg.contains("same type") 
            || msg.contains("Bool operand") 
            || msg.contains("Field operand") 
            || msg.contains("require Field") 
            || msg.contains("require Bool") 
            || msg.contains("must be of type Field") 
            || msg.contains("must be of type Bool")
        {
            "DCL-E105"
        } else if msg.contains("Unknown struct type") || msg.contains("Unknown struct:") {
            "DCL-E106"
        } else if msg.contains("Undefined variable") || msg.contains("Unknown function") {
            "DCL-E107"
        } else if msg.contains("Unsupported integer bit width") {
            "DCL-E110"
        } else {
            "DCL-E100" // General type error
        };
        let err = format!("[Error at line {}, col {}] [{}]: {}", span.line(), span.col(), code, msg);
        self.errors.push(err);
    }

    /// Check if two types are compatible.
    /// Uint(n) is compatible with Field since Uint is a subtype of Field
    /// (constrained to [0, 2^n) range).
    fn types_compatible(&self, a: &Type, b: &Type) -> bool {
        if a == b { return true; }
        // Uint(n) is a subtype of Field
        match (a, b) {
            (Type::Uint(_), Type::Field) | (Type::Field, Type::Uint(_)) => true,
            (Type::Uint(_), Type::Uint(_)) => true, // u8 and u32 are compatible (widening)
            _ => false,
        }
    }

    /// Check an entire module for type correctness.
    ///
    /// Returns `Ok(())` if no errors, or `Err(joined_errors)` if any type violations are found.
    pub fn check_module(&mut self, module: &Module) -> Result<(), String> {
        self.errors.clear();

        // Register all struct types first
        for struct_def in &module.types {
            if self.structs.contains_key(&struct_def.name) {
                self.type_error(struct_def.span, &format!("Duplicate type definition: {}", struct_def.name));
            }
            self.structs.insert(struct_def.name.clone(), struct_def.clone());
        }

        // Detect recursive structs
        for struct_def in &module.types {
            if self.is_recursive_struct(&struct_def.name, &mut Vec::new()) {
                self.type_error(struct_def.span, &format!(
                    "Recursive struct type '{}' has infinite size. Consider using an array or indirection.",
                    struct_def.name
                ));
            }
        }

        // Register all circuits/functions
        for circuit in &module.circuits {
            if self.circuits.contains_key(&circuit.name) {
                self.type_error(circuit.span, &format!("Duplicate circuit definition: {}", circuit.name));
            }
            self.circuits.insert(circuit.name.clone(), circuit.clone());
        }

        // Check each circuit
        for circuit in &module.circuits {
            self.check_circuit(circuit);
        }

        if !self.errors.is_empty() {
            return Err(self.errors.join("\n"));
        }

        Ok(())
    }

    /// Check if a struct type is recursive (contains itself directly or transitively).
    fn is_recursive_struct(&self, name: &str, visited: &mut Vec<String>) -> bool {
        if visited.contains(&name.to_string()) {
            return true;
        }
        visited.push(name.to_string());

        if let Some(def) = self.structs.get(name) {
            for (_, field_ty) in &def.fields {
                if let Type::Struct(ref inner_name) = field_ty {
                    if self.is_recursive_struct(inner_name, visited) {
                        return true;
                    }
                }
            }
        }

        visited.pop();
        false
    }

    fn check_circuit(&mut self, circuit: &Circuit) {
        // Clear variables environment for this circuit
        self.variables.clear();

        // Register parameters
        for param in &circuit.params {
            self.validate_type(&param.ty, circuit.span);
            self.variables.insert(param.name.clone(), (param.ty.clone(), false)); // parameters are immutable
        }

        if circuit.is_extern {
            return;
        }

        // Check statements
        let mut return_type = None;
        for stmt in &circuit.body {
            self.check_statement(stmt, circuit, &mut return_type);
        }

        if return_type.is_none() && !circuit.body.is_empty() {
            // Only allow omitting return for circuits with assert-only bodies
            let has_return = circuit.body.iter().any(|s| matches!(s, Stmt::Return(_, _)));
            if !has_return {
                let has_assert = circuit.body.iter().any(|s| matches!(s, Stmt::Assert(_, _)));
                if !has_assert {
                    self.type_error(circuit.span, &format!("Circuit '{}' missing return statement", circuit.name));
                }
            }
        }
    }

    fn check_statement(&mut self, stmt: &Stmt, circuit: &Circuit, return_type: &mut Option<Type>) {
        match stmt {
            Stmt::Let(name, is_mut, opt_ty, expr, span) => {
                let expr_ty = self.infer_expr_type(expr);
                if let Some(declared_ty) = opt_ty {
                    self.validate_type(declared_ty, *span);
                    if !self.types_compatible(declared_ty, &expr_ty) {
                        self.type_error(*span, &format!(
                            "Declared type {:?} does not match expression type {:?}",
                            declared_ty, expr_ty
                        ));
                    }
                }
                self.variables.insert(name.clone(), (expr_ty, *is_mut));
            }
            Stmt::Assert(expr, span) => {
                let expr_ty = self.infer_expr_type(expr);
                if expr_ty != Type::Bool {
                    self.type_error(*span, &format!("Assertion expression must be Bool, found {:?}", expr_ty));
                }
            }
            Stmt::Assign(lhs, rhs, span) => {
                let lhs_ty = self.infer_expr_type(lhs);
                let rhs_ty = self.infer_expr_type(rhs);
                if lhs_ty != rhs_ty {
                    self.type_error(*span, &format!(
                        "Assignment type mismatch: target is {:?}, expression is {:?}",
                        lhs_ty, rhs_ty
                    ));
                }

                // Verify mutability of target
                match self.get_base_var(lhs) {
                    Ok(base_name) => {
                        if let Some((_, is_mut)) = self.variables.get(&base_name) {
                            if !is_mut {
                                self.type_error(*span, &format!(
                                    "Cannot assign to immutable variable: {}. Use 'let mut' to make it mutable.",
                                    base_name
                                ));
                            }
                        } else {
                            let suggestion = self.suggest_similar_name(&base_name);
                            self.type_error(*span, &format!(
                                "Undefined variable: {}{}",
                                base_name, suggestion
                            ));
                        }
                    }
                    Err(e) => {
                        self.type_error(*span, &e);
                    }
                }
            }
            Stmt::Return(expr, span) => {
                let expr_ty = self.infer_expr_type(expr);
                if expr_ty != circuit.return_ty {
                    self.type_error(*span, &format!(
                        "Circuit return type {:?} does not match expression type {:?}",
                        circuit.return_ty, expr_ty
                    ));
                }
                *return_type = Some(expr_ty);
            }
            Stmt::For(var_name, start_expr, end_expr, body, span) => {
                let start_ty = self.infer_expr_type(start_expr);
                let end_ty = self.infer_expr_type(end_expr);
                if start_ty != Type::Field || end_ty != Type::Field {
                    self.type_error(*span, "Loop range limits must be of type Field");
                }

                let old_val = self.variables.insert(var_name.clone(), (Type::Field, false));
                for s in body {
                    self.check_statement(s, circuit, return_type);
                }
                if let Some(v) = old_val {
                    self.variables.insert(var_name.clone(), v);
                } else {
                    self.variables.remove(var_name);
                }
            }
            Stmt::If(cond, then_body, else_body, span) => {
                let cond_ty = self.infer_expr_type(cond);
                if cond_ty != Type::Bool {
                    self.type_error(*span, &format!("If condition must be Bool, found {:?}", cond_ty));
                }

                let check_block = |tc: &mut Self, block: &Vec<Stmt>, rt: &mut Option<Type>| {
                    let old_vars = tc.variables.clone();
                    for s in block {
                        tc.check_statement(s, circuit, rt);
                    }
                    tc.variables.retain(|k, _| old_vars.contains_key(k));
                };

                check_block(self, then_body, return_type);
                if let Some(else_stmts) = else_body {
                    check_block(self, else_stmts, return_type);
                }
            }
            Stmt::ExprStmt(expr, _span) => {
                // Type-check the expression but discard the result
                self.infer_expr_type(expr);
            }
        }
    }

    fn get_base_var(&self, expr: &Expr) -> Result<String, String> {
        match expr {
            Expr::Var(name, _) => Ok(name.clone()),
            Expr::Access(base, _, _) => self.get_base_var(base),
            Expr::Index(base, _, _) => self.get_base_var(base),
            _ => Err("Invalid assignment target".to_string()),
        }
    }

    fn validate_type(&mut self, ty: &Type, span: Span) {
        match ty {
            Type::Field | Type::Bool => {}
            Type::Uint(bits) => {
                if !matches!(bits, 8 | 16 | 32 | 64) {
                    self.type_error(span, &format!(
                        "Unsupported integer bit width: u{}. Supported: u8, u16, u32, u64", bits
                    ));
                }
            }
            Type::Struct(name) => {
                if !self.structs.contains_key(name) {
                    let suggestion = self.suggest_similar_struct(name);
                    self.type_error(span, &format!("Unknown struct type: {}{}", name, suggestion));
                }
            }
            Type::Array(inner, _) => self.validate_type(inner, span),
        }
    }

    /// Find the most similar variable name for "did you mean?" suggestions.
    fn suggest_similar_name(&self, name: &str) -> String {
        self.find_closest(name, self.variables.keys().map(|s| s.as_str()))
    }

    /// Find the most similar struct name.
    fn suggest_similar_struct(&self, name: &str) -> String {
        self.find_closest(name, self.structs.keys().map(|s| s.as_str()))
    }

    /// Find the most similar function name.
    fn suggest_similar_function(&self, name: &str) -> String {
        self.find_closest(name, self.circuits.keys().map(|s| s.as_str()))
    }

    fn find_closest<'b, I: Iterator<Item = &'b str>>(&self, name: &str, candidates: I) -> String {
        let mut best: Option<(&str, usize)> = None;
        for candidate in candidates {
            let dist = levenshtein_distance(name, candidate);
            if dist <= 3 {
                if best.is_none() || dist < best.unwrap().1 {
                    best = Some((candidate, dist));
                }
            }
        }
        match best {
            Some((suggestion, _)) => format!(". Did you mean '{}'?", suggestion),
            None => String::new(),
        }
    }

    fn infer_expr_type(&mut self, expr: &Expr) -> Type {
        let span = expr.span();
        match expr {
            Expr::Var(name, _) => self
                .variables
                .get(name)
                .map(|(ty, _)| ty.clone())
                .unwrap_or_else(|| {
                    let suggestion = self.suggest_similar_name(name);
                    self.type_error(span, &format!("Undefined variable: {}{}", name, suggestion));
                    Type::Field // fallback
                }),
            Expr::ConstField(_, _) => Type::Field,
            Expr::ConstBool(_, _) => Type::Bool,
            Expr::Unary(op, inner, span) => match op {
                UnOp::Not => {
                    let inner_ty = self.infer_expr_type(inner);
                    if inner_ty != Type::Bool {
                        self.type_error(*span, &format!("Logical Not operator requires Bool operand, found {:?}", inner_ty));
                    }
                    Type::Bool
                }
                UnOp::Neg => {
                    let inner_ty = self.infer_expr_type(inner);
                    if inner_ty != Type::Field {
                        self.type_error(*span, &format!("Negation operator requires Field operand, found {:?}", inner_ty));
                    }
                    Type::Field
                }
            },
            Expr::Binary(op, lhs, rhs, _) => {
                let lhs_ty = self.infer_expr_type(lhs);
                let rhs_ty = self.infer_expr_type(rhs);

                match op {
                    BinOp::And | BinOp::Or => {
                        if lhs_ty != Type::Bool || rhs_ty != Type::Bool {
                            self.type_error(span, &format!("Logical operations require Bool operands, found {:?} and {:?}", lhs_ty, rhs_ty));
                        }
                        Type::Bool
                    }
                    BinOp::Eq | BinOp::NotEq => {
                        if lhs_ty != rhs_ty {
                            self.type_error(span, &format!("Equality comparisons require operands of the same type, found {:?} and {:?}", lhs_ty, rhs_ty));
                        }
                        Type::Bool
                    }
                    BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div => {
                        if lhs_ty != Type::Field || rhs_ty != Type::Field {
                            self.type_error(span, &format!("Arithmetic operations require Field operands, found {:?} and {:?}", lhs_ty, rhs_ty));
                        }
                        Type::Field
                    }
                    BinOp::Gte | BinOp::Lte | BinOp::Lt | BinOp::Gt => {
                        if lhs_ty != Type::Field || rhs_ty != Type::Field {
                            self.type_error(span, &format!("Comparison operations require Field operands, found {:?} and {:?}", lhs_ty, rhs_ty));
                        }
                        Type::Bool
                    }
                }
            }
            Expr::Call(name, args, _) => {
                // 1. Check built-in functions first
                match name.as_str() {
                    "poseidon" | "crypto::poseidon" | "std::crypto::poseidon" => {
                        for arg in args {
                            let arg_ty = self.infer_expr_type(arg);
                            if arg_ty != Type::Field {
                                self.type_error(span, "poseidon arguments must be of type Field");
                            }
                        }
                        return Type::Field;
                    }
                    "range_check" | "utils::range_check" | "std::utils::range_check" => {
                        if args.len() != 2 {
                            self.type_error(span, "range_check expects exactly 2 arguments (value, bits)");
                        } else {
                            let val_ty = self.infer_expr_type(&args[0]);
                            let bits_ty = self.infer_expr_type(&args[1]);
                            if val_ty != Type::Field || bits_ty != Type::Field {
                                self.type_error(span, "range_check arguments must be of type Field");
                            }
                        }
                        return Type::Bool;
                    }
                    _ => {}
                }

                // 2. Look up in registered module/library circuits
                if let Some(target_circuit) = self.circuits.get(name).cloned() {
                    if target_circuit.params.len() != args.len() {
                        self.type_error(span, &format!(
                            "Function '{}' expects {} arguments, found {}",
                            name,
                            target_circuit.params.len(),
                            args.len()
                        ));
                    }
                    for (i, param) in target_circuit.params.iter().enumerate() {
                        if i < args.len() {
                            let arg_ty = self.infer_expr_type(&args[i]);
                            if arg_ty != param.ty {
                                self.type_error(span, &format!(
                                    "Type mismatch for argument {} of '{}': expected {:?}, found {:?}",
                                    i + 1,
                                    name,
                                    param.ty,
                                    arg_ty
                                ));
                            }
                        }
                    }
                    target_circuit.return_ty.clone()
                } else {
                    let suggestion = self.suggest_similar_function(name);
                    self.type_error(span, &format!("Unknown function: {}{}", name, suggestion));
                    Type::Field // fallback
                }
            }
            Expr::Access(base, field_name, _) => {
                let base_ty = self.infer_expr_type(base);
                match base_ty {
                    Type::Struct(struct_name) => {
                        if let Some(struct_def) = self.structs.get(&struct_name).cloned() {
                            for (fname, fty) in &struct_def.fields {
                                if fname == field_name {
                                    return fty.clone();
                                }
                            }
                            let field_names: Vec<&str> = struct_def.fields.iter().map(|(n, _)| n.as_str()).collect();
                            let suggestion = self.find_closest(field_name, field_names.into_iter());
                            self.type_error(span, &format!(
                                "Field '{}' does not exist on struct '{}'{}",
                                field_name, struct_name, suggestion
                            ));
                            Type::Field // fallback
                        } else {
                            self.type_error(span, &format!("Unknown struct: {}", struct_name));
                            Type::Field // fallback
                        }
                    }
                    other => {
                        self.type_error(span, &format!("Cannot access field on non-struct type {:?}", other));
                        Type::Field // fallback
                    }
                }
            }
            Expr::Index(base, index, _) => {
                let base_ty = self.infer_expr_type(base);
                let index_ty = self.infer_expr_type(index);
                if index_ty != Type::Field {
                    self.type_error(span, "Array index must be of type Field");
                }
                match base_ty {
                    Type::Array(inner_ty, _) => *inner_ty,
                    other => {
                        self.type_error(span, &format!("Cannot index into non-array type {:?}", other));
                        Type::Field // fallback
                    }
                }
            }
        }
    }
}

/// Compute the Levenshtein (edit) distance between two strings.
fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_len = a.len();
    let b_len = b.len();
    let mut matrix = vec![vec![0usize; b_len + 1]; a_len + 1];

    for i in 0..=a_len { matrix[i][0] = i; }
    for j in 0..=b_len { matrix[0][j] = j; }

    for (i, ca) in a.chars().enumerate() {
        for (j, cb) in b.chars().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            matrix[i + 1][j + 1] = (matrix[i][j + 1] + 1)
                .min(matrix[i + 1][j] + 1)
                .min(matrix[i][j] + cost);
        }
    }
    matrix[a_len][b_len]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    #[test]
    fn test_typechecker_success() {
        let input = "module Test\ncircuit main(private x: Field) -> bool { assert x > 10; return true; }";
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let module = parser.parse_module().unwrap();
        let mut checker = TypeChecker::new();
        assert!(checker.check_module(&module).is_ok());
    }

    #[test]
    fn test_typechecker_mismatch() {
        let input = "module Test\ncircuit main(private x: Field) -> bool { assert x + 10; return true; }";
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let module = parser.parse_module().unwrap();
        let mut checker = TypeChecker::new();
        assert!(checker.check_module(&module).is_err());
    }

    #[test]
    fn test_did_you_mean_suggestion() {
        let input = "module Test\ncircuit main(private value: Field) -> Field { return vlue; }";
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let module = parser.parse_module().unwrap();
        let mut checker = TypeChecker::new();
        let err = checker.check_module(&module).unwrap_err();
        assert!(err.contains("Did you mean 'value'?"));
    }

    #[test]
    fn test_negation_typecheck() {
        let input = "module Test\ncircuit main(private x: Field) -> Field { return -x; }";
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let module = parser.parse_module().unwrap();
        let mut checker = TypeChecker::new();
        assert!(checker.check_module(&module).is_ok());
    }

    #[test]
    fn test_levenshtein() {
        assert_eq!(levenshtein_distance("kitten", "sitting"), 3);
        assert_eq!(levenshtein_distance("value", "vlue"), 1);
        assert_eq!(levenshtein_distance("abc", "abc"), 0);
    }
}

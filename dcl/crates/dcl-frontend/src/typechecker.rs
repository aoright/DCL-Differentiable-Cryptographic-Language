use crate::ast::*;
use std::collections::HashMap;

pub struct TypeChecker {
    structs: HashMap<String, StructDef>,
    circuits: HashMap<String, Circuit>,
    variables: HashMap<String, (Type, bool)>, // (type, is_mut)
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            structs: HashMap::new(),
            circuits: HashMap::new(),
            variables: HashMap::new(),
        }
    }

    fn type_error<T>(&self, span: Span, msg: &str) -> Result<T, String> {
        Err(format!("[Error at line {}, col {}]: {}", span.line, span.col, msg))
    }

    pub fn check_module(&mut self, module: &Module) -> Result<(), String> {
        // Register all struct types first
        for struct_def in &module.types {
            if self.structs.contains_key(&struct_def.name) {
                return self.type_error(struct_def.span, &format!("Duplicate type definition: {}", struct_def.name));
            }
            self.structs.insert(struct_def.name.clone(), struct_def.clone());
        }

        // Register all circuits/functions
        for circuit in &module.circuits {
            if self.circuits.contains_key(&circuit.name) {
                return self.type_error(circuit.span, &format!("Duplicate circuit definition: {}", circuit.name));
            }
            self.circuits.insert(circuit.name.clone(), circuit.clone());
        }

        // Check each circuit
        for circuit in &module.circuits {
            self.check_circuit(circuit)?;
        }

        Ok(())
    }

    fn check_circuit(&mut self, circuit: &Circuit) -> Result<(), String> {
        // Clear variables environment for this circuit
        self.variables.clear();

        // Register parameters
        for param in &circuit.params {
            self.validate_type(&param.ty, circuit.span)?;
            self.variables.insert(param.name.clone(), (param.ty.clone(), false)); // parameters are immutable
        }

        if circuit.is_extern {
            return Ok(());
        }

        // Check statements
        let mut return_type = None;
        for stmt in &circuit.body {
            self.check_statement(stmt, circuit, &mut return_type)?;
        }

        if return_type.is_none() && circuit.return_ty != Type::Bool {
            return self.type_error(circuit.span, &format!("Circuit '{}' missing return statement", circuit.name));
        }

        Ok(())
    }

    fn check_statement(&mut self, stmt: &Stmt, circuit: &Circuit, return_type: &mut Option<Type>) -> Result<(), String> {
        match stmt {
            Stmt::Let(name, is_mut, opt_ty, expr, span) => {
                let expr_ty = self.infer_expr_type(expr)?;
                if let Some(declared_ty) = opt_ty {
                    self.validate_type(declared_ty, *span)?;
                    if *declared_ty != expr_ty {
                        return self.type_error(*span, &format!(
                            "Declared type {:?} does not match expression type {:?}",
                            declared_ty, expr_ty
                        ));
                    }
                }
                self.variables.insert(name.clone(), (expr_ty, *is_mut));
            }
            Stmt::Assert(expr, span) => {
                let expr_ty = self.infer_expr_type(expr)?;
                if expr_ty != Type::Bool {
                    return self.type_error(*span, &format!("Assertion expression must be Bool, found {:?}", expr_ty));
                }
            }
            Stmt::Assign(lhs, rhs, span) => {
                let lhs_ty = self.infer_expr_type(lhs)?;
                let rhs_ty = self.infer_expr_type(rhs)?;
                if lhs_ty != rhs_ty {
                    return self.type_error(*span, &format!(
                        "Assignment type mismatch: target is {:?}, expression is {:?}",
                        lhs_ty, rhs_ty
                    ));
                }

                // Verify mutability of target
                let base_name = self.get_base_var(lhs)
                    .map_err(|e| format!("[Error at line {}, col {}]: {}", span.line, span.col, e))?;
                if let Some((_, is_mut)) = self.variables.get(&base_name) {
                    if !is_mut {
                        return self.type_error(*span, &format!(
                            "Cannot assign to immutable variable: {}",
                            base_name
                        ));
                    }
                } else {
                    return self.type_error(*span, &format!(
                        "Undefined variable: {}",
                        base_name
                    ));
                }
            }
            Stmt::Return(expr, span) => {
                let expr_ty = self.infer_expr_type(expr)?;
                if expr_ty != circuit.return_ty {
                    return self.type_error(*span, &format!(
                        "Circuit return type {:?} does not match expression type {:?}",
                        circuit.return_ty, expr_ty
                    ));
                }
                *return_type = Some(expr_ty);
            }
            Stmt::For(var_name, start_expr, end_expr, body, span) => {
                let start_ty = self.infer_expr_type(start_expr)?;
                let end_ty = self.infer_expr_type(end_expr)?;
                if start_ty != Type::Field || end_ty != Type::Field {
                    return self.type_error(*span, "Loop range limits must be of type Field");
                }

                let old_val = self.variables.insert(var_name.clone(), (Type::Field, false)); // loop counter is immutable
                for s in body {
                    self.check_statement(s, circuit, return_type)?;
                }
                if let Some(v) = old_val {
                    self.variables.insert(var_name.clone(), v);
                } else {
                    self.variables.remove(var_name);
                }
            }
            Stmt::If(cond, then_body, else_body, span) => {
                let cond_ty = self.infer_expr_type(cond)?;
                if cond_ty != Type::Bool {
                    return self.type_error(*span, &format!("If condition must be Bool, found {:?}", cond_ty));
                }

                // Helper to check block statement scope
                let check_block = |tc: &mut Self, block: &Vec<Stmt>, rt: &mut Option<Type>| -> Result<(), String> {
                    let old_vars = tc.variables.clone();
                    for s in block {
                        tc.check_statement(s, circuit, rt)?;
                    }
                    tc.variables.retain(|k, _| old_vars.contains_key(k));
                    Ok(())
                };

                check_block(self, then_body, return_type)?;
                if let Some(else_stmts) = else_body {
                    check_block(self, else_stmts, return_type)?;
                }
            }
        }
        Ok(())
    }

    fn get_base_var(&self, expr: &Expr) -> Result<String, String> {
        match expr {
            Expr::Var(name, _) => Ok(name.clone()),
            Expr::Access(base, _, _) => self.get_base_var(base),
            Expr::Index(base, _, _) => self.get_base_var(base),
            _ => Err("Invalid assignment target".to_string()),
        }
    }

    fn validate_type(&self, ty: &Type, span: Span) -> Result<(), String> {
        match ty {
            Type::Field | Type::Bool => Ok(()),
            Type::Struct(name) => {
                if !self.structs.contains_key(name) {
                    self.type_error(span, &format!("Unknown struct type: {}", name))
                } else {
                    Ok(())
                }
            }
            Type::Array(inner, _) => self.validate_type(inner, span),
        }
    }

    fn infer_expr_type(&self, expr: &Expr) -> Result<Type, String> {
        let span = expr.span();
        match expr {
            Expr::Var(name, _) => self
                .variables
                .get(name)
                .map(|(ty, _)| ty.clone())
                .ok_or_else(|| format!("[Error at line {}, col {}]: Undefined variable: {}", span.line, span.col, name)),
            Expr::ConstField(_, _) => Ok(Type::Field),
            Expr::ConstBool(_, _) => Ok(Type::Bool),
            Expr::Unary(op, inner, span) => match op {
                crate::ast::UnOp::Not => {
                    let inner_ty = self.infer_expr_type(inner)?;
                    if inner_ty != Type::Bool {
                        return self.type_error(*span, &format!("Logical Not operator requires Bool operand, found {:?}", inner_ty));
                    }
                    Ok(Type::Bool)
                }
            },
            Expr::Binary(op, lhs, rhs, _) => {
                let lhs_ty = self.infer_expr_type(lhs)?;
                let rhs_ty = self.infer_expr_type(rhs)?;

                match op {
                    BinOp::And | BinOp::Or => {
                        if lhs_ty != Type::Bool || rhs_ty != Type::Bool {
                            return self.type_error(span, &format!("Logical operations require Bool operands, found {:?} and {:?}", lhs_ty, rhs_ty));
                        }
                        Ok(Type::Bool)
                    }
                    BinOp::Eq | BinOp::NotEq => {
                        if lhs_ty != rhs_ty {
                            return self.type_error(span, &format!("Equality comparisons require operands of the same type, found {:?} and {:?}", lhs_ty, rhs_ty));
                        }
                        Ok(Type::Bool)
                    }
                    BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div => {
                        if lhs_ty != Type::Field || rhs_ty != Type::Field {
                            return self.type_error(span, &format!("Arithmetic operations require Field operands, found {:?} and {:?}", lhs_ty, rhs_ty));
                        }
                        Ok(Type::Field)
                    }
                    BinOp::Gte | BinOp::Lte | BinOp::Lt | BinOp::Gt => {
                        if lhs_ty != Type::Field || rhs_ty != Type::Field {
                            return self.type_error(span, &format!("Comparison operations require Field operands, found {:?} and {:?}", lhs_ty, rhs_ty));
                        }
                        Ok(Type::Bool)
                    }
                }
            }
            Expr::Call(name, args, _) => {
                // 1. Check built-in functions first
                match name.as_str() {
                    "poseidon" | "crypto::poseidon" | "std::crypto::poseidon" => {
                        for arg in args {
                            let arg_ty = self.infer_expr_type(arg)?;
                            if arg_ty != Type::Field {
                                return self.type_error(span, "poseidon arguments must be of type Field");
                            }
                        }
                        return Ok(Type::Field);
                    }
                    "range_check" | "utils::range_check" | "std::utils::range_check" => {
                        if args.len() != 2 {
                            return self.type_error(span, "range_check expects exactly 2 arguments (value, bits)");
                        }
                        let val_ty = self.infer_expr_type(&args[0])?;
                        let bits_ty = self.infer_expr_type(&args[1])?;
                        if val_ty != Type::Field || bits_ty != Type::Field {
                            return self.type_error(span, "range_check arguments must be of type Field");
                        }
                        return Ok(Type::Bool);
                    }
                    _ => {}
                }

                // 2. Look up in registered module/library circuits
                if let Some(target_circuit) = self.circuits.get(name) {
                    if target_circuit.params.len() != args.len() {
                        return self.type_error(span, &format!(
                            "Function '{}' expects {} arguments, found {}",
                            name,
                            target_circuit.params.len(),
                            args.len()
                        ));
                    }
                    for (i, param) in target_circuit.params.iter().enumerate() {
                        let arg_ty = self.infer_expr_type(&args[i])?;
                        if arg_ty != param.ty {
                            return self.type_error(span, &format!(
                                "Type mismatch for argument {} of '{}': expected {:?}, found {:?}",
                                i + 1,
                                name,
                                param.ty,
                                arg_ty
                            ));
                        }
                    }
                    Ok(target_circuit.return_ty.clone())
                } else {
                    self.type_error(span, &format!("Unknown function: {}", name))
                }
            }
            Expr::Access(base, field_name, _) => {
                let base_ty = self.infer_expr_type(base)?;
                match base_ty {
                    Type::Struct(struct_name) => {
                        let struct_def = self
                            .structs
                            .get(&struct_name)
                            .ok_or_else(|| format!("[Error at line {}, col {}]: Unknown struct: {}", span.line, span.col, struct_name))?;
                        for (fname, fty) in &struct_def.fields {
                            if fname == field_name {
                                return Ok(fty.clone());
                            }
                        }
                        self.type_error(span, &format!(
                            "Field '{}' does not exist on struct '{}'",
                            field_name, struct_name
                        ))
                    }
                    other => self.type_error(span, &format!("Cannot access field on non-struct type {:?}", other)),
                }
            }
            Expr::Index(base, index, _) => {
                let base_ty = self.infer_expr_type(base)?;
                let index_ty = self.infer_expr_type(index)?;
                if index_ty != Type::Field {
                    return self.type_error(span, "Array index must be of type Field");
                }
                match base_ty {
                    Type::Array(inner_ty, _) => Ok(*inner_ty),
                    other => self.type_error(span, &format!("Cannot index into non-array type {:?}", other)),
                }
            }
        }
    }
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
        // Assert expects a boolean, but x + 10 is a Field
        let input = "module Test\ncircuit main(private x: Field) -> bool { assert x + 10; return true; }";
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let module = parser.parse_module().unwrap();
        let mut checker = TypeChecker::new();
        assert!(checker.check_module(&module).is_err());
    }
}

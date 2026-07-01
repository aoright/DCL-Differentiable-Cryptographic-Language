use crate::ast::{Module, StructDef, Circuit, Stmt, Expr, BinOp, UnOp, Type};

pub fn format_module(module: &Module) -> String {
    let mut s = String::new();

    // 1. Format module header
    s.push_str(&format!("module {}\n\n", module.name));

    // 2. Format imports
    if !module.imports.is_empty() {
        for import in &module.imports {
            if !import.is_empty() {
                s.push_str(&format!("use {};\n", import.join("::")));
            }
        }
        s.push_str("\n");
    }

    // 3. Format types (Structs)
    for tdef in &module.types {
        s.push_str(&format_struct_def(tdef));
    }

    // 4. Format circuits
    for circ in &module.circuits {
        s.push_str(&format_circuit(circ));
    }

    s
}

fn format_struct_def(def: &StructDef) -> String {
    let mut s = format!("type {} = {{\n", def.name);
    for (name, ty) in &def.fields {
        s.push_str(&format!("    {}: {},\n", name, format_type(ty)));
    }
    s.push_str("}\n\n");
    s
}

fn format_circuit(c: &Circuit) -> String {
    let mut s = String::new();
    if c.is_extern {
        s.push_str("extern ");
    }
    s.push_str(&format!("circuit {}(\n", c.name));
    
    let mut params_str = Vec::new();
    for param in &c.params {
        let vis_str = match param.visibility {
            crate::ast::Visibility::Private => "private ",
            crate::ast::Visibility::Public => "public ",
            crate::ast::Visibility::Shared => "shared ",
        };
        params_str.push(format!("    {}{}: {}", vis_str, param.name, format_type(&param.ty)));
    }
    s.push_str(&params_str.join(",\n"));
    s.push_str("\n)");

    if c.is_extern {
        s.push_str(&format!(" -> {};\n\n", format_type(&c.return_ty)));
    } else {
        s.push_str(&format!(" -> {} {{\n", format_type(&c.return_ty)));
        for stmt in &c.body {
            s.push_str(&format_stmt(stmt, 4));
        }
        s.push_str("}\n\n");
    }
    s
}

fn format_type(ty: &Type) -> String {
    match ty {
        Type::Field => "Field".to_string(),
        Type::Bool => "bool".to_string(),
        Type::Struct(name) => name.clone(),
        Type::Array(inner, size) => format!("{}[{}]", format_type(inner), size),
    }
}

fn format_stmt(stmt: &Stmt, indent: usize) -> String {
    let spaces = " ".repeat(indent);
    match stmt {
        Stmt::Let(name, is_mut, ty, expr, _) => {
            let mut s = format!("{}let {}{}", spaces, if *is_mut { "mut " } else { "" }, name);
            if let Some(t) = ty {
                s.push_str(&format!(": {}", format_type(t)));
            }
            s.push_str(&format!(" = {};\n", format_expr(expr)));
            s
        }
        Stmt::Assert(expr, _) => {
            format!("{}assert {};\n", spaces, format_expr(expr))
        }
        Stmt::Assign(lhs, rhs, _) => {
            format!("{}{} = {};\n", spaces, format_expr(lhs), format_expr(rhs))
        }
        Stmt::Return(expr, _) => {
            format!("{}return {};\n", spaces, format_expr(expr))
        }
        Stmt::For(var, start, end, body, _) => {
            let mut s = format!("{}for {} in {}..{} {{\n", spaces, var, format_expr(start), format_expr(end));
            for st in body {
                s.push_str(&format_stmt(st, indent + 4));
            }
            s.push_str(&format!("{}}}\n", spaces));
            s
        }
        Stmt::If(cond, then_body, else_body, _) => {
            let mut s = format!("{}if {} {{\n", spaces, format_expr(cond));
            for st in then_body {
                s.push_str(&format_stmt(st, indent + 4));
            }
            if let Some(else_stmts) = else_body {
                s.push_str(&format!("{}}} else {{\n", spaces));
                for st in else_stmts {
                    s.push_str(&format_stmt(st, indent + 4));
                }
            }
            s.push_str(&format!("{}}}\n", spaces));
            s
        }
        Stmt::ExprStmt(expr, _) => {
            format!("{}{};\n", spaces, format_expr(expr))
        }
    }
}

fn op_precedence(op: &BinOp) -> i8 {
    match op {
        BinOp::And | BinOp::Or => 0,
        BinOp::Eq | BinOp::NotEq | BinOp::Gte | BinOp::Lte | BinOp::Lt | BinOp::Gt => 1,
        BinOp::Add | BinOp::Sub => 2,
        BinOp::Mul | BinOp::Div => 3,
    }
}

fn expr_precedence(expr: &Expr) -> i8 {
    match expr {
        Expr::Binary(op, _, _, _) => op_precedence(op),
        _ => 10,
    }
}

fn format_binary_operand(expr: &Expr, parent_prec: i8, is_right: bool) -> String {
    let prec = expr_precedence(expr);
    let needs_paren = if is_right {
        prec <= parent_prec
    } else {
        prec < parent_prec
    };
    if needs_paren {
        format!("({})", format_expr(expr))
    } else {
        format_expr(expr)
    }
}

fn format_expr(expr: &Expr) -> String {
    match expr {
        Expr::Var(name, _) => name.clone(),
        Expr::ConstField(val, _) => val.clone(),
        Expr::ConstBool(val, _) => val.to_string(),
        Expr::Unary(op, inner, _) => {
            let op_str = match op {
                UnOp::Not => "!",
                UnOp::Neg => "-",
            };
            format!("{}{}", op_str, format_expr(inner))
        }
        Expr::Binary(op, lhs, rhs, _) => {
            let parent_prec = op_precedence(op);
            let op_str = match op {
                BinOp::Add => "+",
                BinOp::Sub => "-",
                BinOp::Mul => "*",
                BinOp::Div => "/",
                BinOp::Eq => "==",
                BinOp::NotEq => "!=",
                BinOp::Gte => ">=",
                BinOp::Lte => "<=",
                BinOp::Lt => "<",
                BinOp::Gt => ">",
                BinOp::And => "&&",
                BinOp::Or => "||",
            };
            format!(
                "{} {} {}",
                format_binary_operand(lhs, parent_prec, false),
                op_str,
                format_binary_operand(rhs, parent_prec, true)
            )
        }
        Expr::Call(name, args, _) => {
            let formatted_args = args.iter().map(format_expr).collect::<Vec<_>>().join(", ");
            format!("{}({})", name, formatted_args)
        }
        Expr::Access(base, field, _) => {
            format!("{}.{}", format_expr(base), field)
        }
        Expr::Index(base, index, _) => {
            format!("{}[{}]", format_expr(base), format_expr(index))
        }
    }
}

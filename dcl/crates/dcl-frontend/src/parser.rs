use crate::ast::*;
use crate::lexer::{Token, TokenWithSpan};

pub struct Parser {
    tokens: Vec<TokenWithSpan>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<TokenWithSpan>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos).map(|t| &t.token)
    }

    fn peek_span(&self) -> Span {
        self.tokens.get(self.pos).map(|t| t.span).unwrap_or(Span::new(0, 0))
    }

    fn next_token(&mut self) -> Option<Token> {
        if self.pos < self.tokens.len() {
            let tok = self.tokens[self.pos].token.clone();
            self.pos += 1;
            Some(tok)
        } else {
            None
        }
    }

    fn next_token_with_span(&mut self) -> Option<TokenWithSpan> {
        if self.pos < self.tokens.len() {
            let tok = self.tokens[self.pos].clone();
            self.pos += 1;
            Some(tok)
        } else {
            None
        }
    }

    fn expect(&mut self, expected: Token) -> Result<(), String> {
        let span = self.peek_span();
        match self.next_token() {
            Some(tok) if tok == expected => Ok(()),
            other => Err(format!("[Error at line {}, col {}]: Expected {:?}, found {:?}", span.line, span.col, expected, other)),
        }
    }

    fn parse_type(&mut self) -> Result<Type, String> {
        let tok = self.peek().ok_or("Unexpected end of input reading type")?;
        match tok {
            Token::LBracket => {
                self.next_token(); // consume '['
                let inner_ty = self.parse_type()?;
                self.expect(Token::Semicolon)?;
                let span = self.peek_span();
                let len_tok = self.next_token().ok_or("Expected array size")?;
                let len = match len_tok {
                    Token::Num(s) => s.parse::<usize>().map_err(|_| format!("[Error at line {}, col {}]: Invalid array size: {}", span.line, span.col, s))?,
                    other => return Err(format!("[Error at line {}, col {}]: Expected number for array size, found {:?}", span.line, span.col, other)),
                };
                self.expect(Token::RBracket)?;
                Ok(Type::Array(Box::new(inner_ty), len))
            }
            _ => {
                let span = self.peek_span();
                let base_tok = self.next_token().ok_or("Unexpected end of input reading type")?;
                let base_ty = match base_tok {
                    Token::FieldTy => Type::Field,
                    Token::BoolTy => Type::Bool,
                    Token::Ident(name) => Type::Struct(name),
                    other => return Err(format!("[Error at line {}, col {}]: Expected type keyword or identifier, found {:?}", span.line, span.col, other)),
                };

                if self.peek() == Some(&Token::LBracket) {
                    self.next_token(); // consume '['
                    let len_span = self.peek_span();
                    let len_tok = self.next_token().ok_or("Expected array size")?;
                    let len = match len_tok {
                        Token::Num(s) => s.parse::<usize>().map_err(|_| format!("[Error at line {}, col {}]: Invalid array size: {}", len_span.line, len_span.col, s))?,
                        other => return Err(format!("[Error at line {}, col {}]: Expected number for array size, found {:?}", len_span.line, len_span.col, other)),
                    };
                    self.expect(Token::RBracket)?;
                    Ok(Type::Array(Box::new(base_ty), len))
                } else {
                    Ok(base_ty)
                }
            }
        }
    }

    pub fn parse_module(&mut self) -> Result<Module, String> {
        self.expect(Token::Module)?;
        let mut path = Vec::new();
        let span = self.peek_span();
        let name_tok = self.next_token().ok_or("Expected module name")?;
        match name_tok {
            Token::Ident(name) => path.push(name),
            other => return Err(format!("[Error at line {}, col {}]: Expected module identifier, found {:?}", span.line, span.col, other)),
        }
        while self.peek() == Some(&Token::DoubleColon) {
            self.next_token(); // consume '::'
            let next_span = self.peek_span();
            let next_tok = self.next_token().ok_or("Expected identifier after '::'")?;
            match next_tok {
                Token::Ident(name) => path.push(name),
                other => return Err(format!("[Error at line {}, col {}]: Expected identifier, found {:?}", next_span.line, next_span.col, other)),
            }
        }
        let module_name = path.join("::");

        // Note: semicolon at end of module name might be optional or omitted.
        if self.peek() == Some(&Token::Semicolon) {
            self.next_token();
        }

        let mut imports = Vec::new();
        let mut types = Vec::new();
        let mut circuits = Vec::new();

        while let Some(tok) = self.peek() {
            match tok {
                Token::Use => {
                    self.next_token(); // consume 'use'
                    let mut path = Vec::new();
                    let first_span = self.peek_span();
                    let first_tok = self.next_token().ok_or("Expected identifier in import path")?;
                    match first_tok {
                        Token::Ident(name) => path.push(name),
                        other => return Err(format!("[Error at line {}, col {}]: Expected identifier, found {:?}", first_span.line, first_span.col, other)),
                    }
                    while self.peek() == Some(&Token::DoubleColon) {
                        self.next_token(); // consume '::'
                        let next_span = self.peek_span();
                        let next_tok = self.next_token().ok_or("Expected identifier after '::'")?;
                        match next_tok {
                            Token::Ident(name) => path.push(name),
                            other => return Err(format!("[Error at line {}, col {}]: Expected identifier, found {:?}", next_span.line, next_span.col, other)),
                        }
                    }
                    if self.peek() == Some(&Token::Semicolon) {
                        self.next_token();
                    }
                    imports.push(path);
                }
                Token::Type => {
                    self.next_token(); // consume 'type'
                    let struct_def = self.parse_struct_def()?;
                    types.push(struct_def);
                }
                Token::Circuit | Token::Extern => {
                    let is_extern = if self.peek() == Some(&Token::Extern) {
                        self.next_token(); // consume 'extern'
                        true
                    } else {
                        false
                    };
                    self.expect(Token::Circuit)?;
                    let circuit = self.parse_circuit(is_extern)?;
                    circuits.push(circuit);
                }
                other => return Err(format!("[Error at line {}, col {}]: Expected 'use', 'type', 'extern', or 'circuit', found {:?}", self.peek_span().line, self.peek_span().col, other)),
            }
        }

        Ok(Module {
            name: module_name,
            imports,
            types,
            circuits,
        })
    }

    fn parse_struct_def(&mut self) -> Result<StructDef, String> {
        let span = self.peek_span();
        let name_tok = self.next_token().ok_or("Expected struct name")?;
        let name = match name_tok {
            Token::Ident(n) => n,
            other => return Err(format!("[Error at line {}, col {}]: Expected struct identifier, found {:?}", span.line, span.col, other)),
        };

        self.expect(Token::Eq)?;
        self.expect(Token::LBrace)?;

        let mut fields = Vec::new();
        while self.peek() != Some(&Token::RBrace) {
            let field_span = self.peek_span();
            let field_name_tok = self.next_token().ok_or("Expected field name")?;
            let field_name = match field_name_tok {
                Token::Ident(n) => n,
                other => return Err(format!("[Error at line {}, col {}]: Expected field identifier, found {:?}", field_span.line, field_span.col, other)),
            };

            self.expect(Token::Colon)?;
            let field_ty = self.parse_type()?;

            fields.push((field_name, field_ty));

            if self.peek() == Some(&Token::Comma) {
                self.next_token();
            } else if self.peek() != Some(&Token::RBrace) {
                return Err(format!("[Error at line {}, col {}]: Expected ',' or '}}', found {:?}", self.peek_span().line, self.peek_span().col, self.peek()));
            }
        }
        self.expect(Token::RBrace)?;

        Ok(StructDef { name, fields, span })
    }

    fn parse_circuit(&mut self, is_extern: bool) -> Result<Circuit, String> {
        let span = self.peek_span();
        let name_tok = self.next_token().ok_or("Expected circuit name")?;
        let name = match name_tok {
            Token::Ident(n) => n,
            other => return Err(format!("[Error at line {}, col {}]: Expected circuit identifier, found {:?}", span.line, span.col, other)),
        };

        self.expect(Token::LParen)?;
        let mut params = Vec::new();
        while self.peek() != Some(&Token::RParen) {
            let visibility = match self.peek() {
                Some(Token::Private) => {
                    self.next_token();
                    Visibility::Private
                }
                Some(Token::Public) => {
                    self.next_token();
                    Visibility::Public
                }
                Some(Token::Shared) => {
                    self.next_token();
                    Visibility::Shared
                }
                _ => Visibility::Private, // default to private
            };

            let param_span = self.peek_span();
            let param_name_tok = self.next_token().ok_or("Expected parameter name")?;
            let param_name = match param_name_tok {
                Token::Ident(n) => n,
                other => return Err(format!("[Error at line {}, col {}]: Expected parameter identifier, found {:?}", param_span.line, param_span.col, other)),
            };

            self.expect(Token::Colon)?;
            let param_ty = self.parse_type()?;

            params.push(Parameter {
                name: param_name,
                visibility,
                ty: param_ty,
            });

            if self.peek() == Some(&Token::Comma) {
                self.next_token();
            } else if self.peek() != Some(&Token::RParen) {
                return Err(format!("[Error at line {}, col {}]: Expected ',' or ')', found {:?}", self.peek_span().line, self.peek_span().col, self.peek()));
            }
        }
        self.expect(Token::RParen)?;

        self.expect(Token::Arrow)?;
        let return_ty = self.parse_type()?;

        let body = if is_extern {
            if self.peek() == Some(&Token::Semicolon) {
                self.next_token();
            }
            Vec::new()
        } else {
            self.expect(Token::LBrace)?;
            let mut body = Vec::new();
            while self.peek() != Some(&Token::RBrace) {
                let stmt = self.parse_statement()?;
                body.push(stmt);
            }
            self.expect(Token::RBrace)?;
            body
        };

        Ok(Circuit {
            name,
            params,
            return_ty,
            body,
            is_extern,
            span,
        })
    }

    fn parse_statement(&mut self) -> Result<Stmt, String> {
        let span = self.peek_span();
        let tok = self.peek().cloned().ok_or("Unexpected end of input reading statement")?;
        let stmt = match &tok {
            Token::Let => {
                self.next_token(); // consume 'let'
                let mut is_mut = false;
                if self.peek() == Some(&Token::Mut) {
                    self.next_token(); // consume 'mut'
                    is_mut = true;
                }
                let var_span = self.peek_span();
                let var_name_tok = self.next_token().ok_or("Expected variable name")?;
                let var_name = match var_name_tok {
                    Token::Ident(n) => n,
                    other => return Err(format!("[Error at line {}, col {}]: Expected identifier after 'let', found {:?}", var_span.line, var_span.col, other)),
                };

                let mut var_ty = None;
                if self.peek() == Some(&Token::Colon) {
                    self.next_token(); // consume ':'
                    var_ty = Some(self.parse_type()?);
                }

                self.expect(Token::Eq)?;
                let expr = self.parse_expr()?;
                Stmt::Let(var_name, is_mut, var_ty, expr, span)
            }
            Token::Assert => {
                self.next_token(); // consume 'assert'
                let expr = self.parse_expr()?;
                Stmt::Assert(expr, span)
            }
            Token::Return => {
                self.next_token(); // consume 'return'
                let expr = self.parse_expr()?;
                Stmt::Return(expr, span)
            }
            Token::For => {
                self.next_token(); // consume 'for'
                let var_span = self.peek_span();
                let var_tok = self.next_token().ok_or("Expected loop variable name")?;
                let var_name = match var_tok {
                    Token::Ident(n) => n,
                    other => return Err(format!("[Error at line {}, col {}]: Expected loop variable identifier, found {:?}", var_span.line, var_span.col, other)),
                };
                self.expect(Token::In)?;
                let start_expr = self.parse_expr()?;
                self.expect(Token::DotDot)?;
                let end_expr = self.parse_expr()?;

                self.expect(Token::LBrace)?;
                let mut body = Vec::new();
                while self.peek() != Some(&Token::RBrace) {
                    body.push(self.parse_statement()?);
                }
                self.expect(Token::RBrace)?;

                Stmt::For(var_name, Box::new(start_expr), Box::new(end_expr), body, span)
            }
            Token::If => {
                self.next_token(); // consume 'if'
                let cond = self.parse_expr()?;
                self.expect(Token::LBrace)?;
                let mut then_body = Vec::new();
                while self.peek() != Some(&Token::RBrace) {
                    then_body.push(self.parse_statement()?);
                }
                self.expect(Token::RBrace)?;

                let mut else_body = None;
                if self.peek() == Some(&Token::Else) {
                    self.next_token(); // consume 'else'
                    if self.peek() == Some(&Token::LBrace) {
                        self.next_token(); // consume '{'
                        let mut else_stmts = Vec::new();
                        while self.peek() != Some(&Token::RBrace) {
                            else_stmts.push(self.parse_statement()?);
                        }
                        self.expect(Token::RBrace)?;
                        else_body = Some(else_stmts);
                    } else if self.peek() == Some(&Token::If) {
                        let stmt = self.parse_statement()?;
                        else_body = Some(vec![stmt]);
                    } else {
                        return Err(format!("[Error at line {}, col {}]: Expected '{{' or 'if' after 'else'", self.peek_span().line, self.peek_span().col));
                    }
                }

                Stmt::If(Box::new(cond), then_body, else_body, span)
            }
            _ => {
                let lhs = self.parse_expr()?;
                if self.peek() == Some(&Token::Eq) {
                    self.next_token(); // consume '='
                    let rhs = self.parse_expr()?;
                    Stmt::Assign(Box::new(lhs), rhs, span)
                } else {
                    return Err(format!("[Error at line {}, col {}]: Expected statement, found {:?}", span.line, span.col, tok));
                }
            }
        };

        // Semicolon might follow the statement
        if self.peek() == Some(&Token::Semicolon) {
            self.next_token();
        }
        Ok(stmt)
    }

    fn parse_expr(&mut self) -> Result<Expr, String> {
        self.parse_binary_expr(0)
    }

    fn parse_binary_expr(&mut self, min_prec: i8) -> Result<Expr, String> {
        let mut lhs = self.parse_primary_expr()?;

        while let Some(tok) = self.peek() {
            let op = match tok {
                Token::Plus => BinOp::Add,
                Token::Minus => BinOp::Sub,
                Token::Mul => BinOp::Mul,
                Token::Div => BinOp::Div,
                Token::DoubleEq => BinOp::Eq,
                Token::NotEq => BinOp::NotEq,
                Token::Gte => BinOp::Gte,
                Token::Lte => BinOp::Lte,
                Token::Lt => BinOp::Lt,
                Token::Gt => BinOp::Gt,
                Token::And => BinOp::And,
                Token::Or => BinOp::Or,
                _ => break,
            };

            let prec = self.op_precedence(op);
            if prec < min_prec {
                break;
            }

            self.next_token(); // consume operator
            let rhs = self.parse_binary_expr(prec + 1)?;
            let span = lhs.span();
            lhs = Expr::Binary(op, Box::new(lhs), Box::new(rhs), span);
        }

        Ok(lhs)
    }

    fn op_precedence(&self, op: BinOp) -> i8 {
        match op {
            BinOp::And | BinOp::Or => 0,
            BinOp::Eq | BinOp::NotEq | BinOp::Gte | BinOp::Lte | BinOp::Lt | BinOp::Gt => 1,
            BinOp::Add | BinOp::Sub => 2,
            BinOp::Mul | BinOp::Div => 3,
        }
    }

    fn parse_primary_expr(&mut self) -> Result<Expr, String> {
        let span = self.peek_span();
        let tok_with_span = self.next_token_with_span().ok_or("Unexpected end of input reading expression")?;
        let tok = tok_with_span.token;
        let mut expr = match tok {
            Token::Ident(first_name) => {
                let mut path = vec![first_name];
                while self.peek() == Some(&Token::DoubleColon) {
                    self.next_token(); // consume '::'
                    let next_span = self.peek_span();
                    let next_tok = self.next_token().ok_or("Expected identifier after '::'")?;
                    match next_tok {
                        Token::Ident(n) => path.push(n),
                        other => return Err(format!("[Error at line {}, col {}]: Expected identifier, found {:?}", next_span.line, next_span.col, other)),
                    }
                }

                if self.peek() == Some(&Token::LParen) {
                    let joined_name = path.join("::");
                    self.next_token(); // consume '('
                    let mut args = Vec::new();
                    while self.peek() != Some(&Token::RParen) {
                        args.push(self.parse_expr()?);
                        if self.peek() == Some(&Token::Comma) {
                            self.next_token();
                        } else if self.peek() != Some(&Token::RParen) {
                            return Err(format!("[Error at line {}, col {}]: Expected ',' or ')', found {:?}", self.peek_span().line, self.peek_span().col, self.peek()));
                        }
                    }
                    self.expect(Token::RParen)?;
                    Expr::Call(joined_name, args, span)
                } else {
                    let joined_name = path.join("::");
                    Expr::Var(joined_name, span)
                }
            }
            Token::Not => {
                let inner = self.parse_primary_expr()?;
                Expr::Unary(crate::ast::UnOp::Not, Box::new(inner), span)
            }
            Token::Num(val) => Expr::ConstField(val, span),
            Token::Bool(val) => Expr::ConstBool(val, span),
            Token::LParen => {
                let e = self.parse_expr()?;
                self.expect(Token::RParen)?;
                e
            }
            other => return Err(format!("[Error at line {}, col {}]: Unexpected token in expression: {:?}", span.line, span.col, other)),
        };

        // Check for field access like x.y or array indexing (which we can treat as access/indexing)
        while self.peek() == Some(&Token::Dot) || self.peek() == Some(&Token::LBracket) {
            let current_span = expr.span();
            if self.peek() == Some(&Token::Dot) {
                self.next_token(); // consume '.'
                let field_span = self.peek_span();
                let field_tok = self.next_token().ok_or("Expected field identifier after '.'")?;
                let field_name = match field_tok {
                    Token::Ident(n) => n,
                    other => return Err(format!("[Error at line {}, col {}]: Expected field identifier, found {:?}", field_span.line, field_span.col, other)),
                };
                expr = Expr::Access(Box::new(expr), field_name, current_span);
            } else {
                self.next_token(); // consume '['
                let idx_expr = self.parse_expr()?;
                self.expect(Token::RBracket)?;
                expr = Expr::Index(Box::new(expr), Box::new(idx_expr), current_span);
            }
        }

        Ok(expr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;

    #[test]
    fn test_parser_basic() {
        let input = "module Test\ncircuit main(private x: Field) -> bool { assert x > 10; return true; }";
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let module = parser.parse_module().unwrap();
        assert_eq!(module.name, "Test");
        assert_eq!(module.circuits.len(), 1);
        let circuit = &module.circuits[0];
        assert_eq!(circuit.name, "main");
        assert_eq!(circuit.params.len(), 1);
        assert_eq!(circuit.params[0].name, "x");
        assert_eq!(circuit.params[0].visibility, Visibility::Private);
    }
}

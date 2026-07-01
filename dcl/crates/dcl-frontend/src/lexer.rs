use crate::ast::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Keywords
    Module,
    Type,
    Circuit,
    Private,
    Public,
    Shared,
    Let,
    Assert,
    Return,
    FieldTy,
    BoolTy,
    For,
    In,
    If,
    Else,
    Mut,
    Use,
    Extern,

    // Identifiers & Literals
    Ident(String),
    Bool(bool),
    Num(String),

    // Symbols & Operators
    Plus,
    Minus,
    Mul,
    Div,
    DoubleEq,
    Gte,
    Lte,
    Lt,
    Gt,
    Eq,
    Arrow,
    DotDot,
    DoubleColon,
    And,
    Or,
    Not,
    NotEq,

    // Delimiters
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Colon,
    Dot,
    Semicolon,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TokenWithSpan {
    pub token: Token,
    pub span: Span,
}

pub struct Lexer<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
    line: usize,
    col: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            chars: input.chars().peekable(),
            line: 1,
            col: 1,
        }
    }

    fn next_char(&mut self) -> Option<char> {
        let c = self.chars.next()?;
        if c == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(c)
    }

    pub fn tokenize(&mut self) -> Result<Vec<TokenWithSpan>, String> {
        let mut tokens = Vec::new();
        while let Some(&c) = self.chars.peek() {
            let start_line = self.line;
            let start_col = self.col;

            if c.is_whitespace() {
                self.next_char();
                continue;
            }

            if c == '/' {
                self.next_char();
                if self.chars.peek() == Some(&'/') {
                    // Comment line, skip it
                    while let Some(nc) = self.next_char() {
                        if nc == '\n' {
                            break;
                        }
                    }
                    continue;
                } else {
                    let span = Span::new(start_line, start_col);
                    tokens.push(TokenWithSpan { token: Token::Div, span });
                    continue;
                }
            }

            if c.is_alphabetic() || c == '_' {
                let mut ident = String::new();
                while let Some(&nc) = self.chars.peek() {
                    if nc.is_alphanumeric() || nc == '_' {
                        ident.push(nc);
                        self.next_char();
                    } else {
                        break;
                    }
                }
                let token = match ident.as_str() {
                    "module" => Token::Module,
                    "type" => Token::Type,
                    "circuit" => Token::Circuit,
                    "private" => Token::Private,
                    "public" => Token::Public,
                    "shared" => Token::Shared,
                    "let" => Token::Let,
                    "assert" => Token::Assert,
                    "return" => Token::Return,
                    "Field" => Token::FieldTy,
                    "bool" => Token::BoolTy,
                    "true" => Token::Bool(true),
                    "false" => Token::Bool(false),
                    "for" => Token::For,
                    "in" => Token::In,
                    "if" => Token::If,
                    "else" => Token::Else,
                    "mut" => Token::Mut,
                    "use" => Token::Use,
                    "extern" => Token::Extern,
                    _ => Token::Ident(ident),
                };
                let span = Span::new(start_line, start_col);
                tokens.push(TokenWithSpan { token, span });
                continue;
            }

            if c.is_numeric() {
                let mut num_str = String::new();
                while let Some(&nc) = self.chars.peek() {
                    if nc.is_numeric() {
                        num_str.push(nc);
                        self.next_char();
                    } else if nc == '.' {
                        let mut peek_chars = self.chars.clone();
                        peek_chars.next();
                        if peek_chars.peek() == Some(&'.') {
                            break;
                        } else {
                            num_str.push(nc);
                            self.next_char();
                        }
                    } else {
                        break;
                    }
                }
                let span = Span::new(start_line, start_col);
                tokens.push(TokenWithSpan { token: Token::Num(num_str), span });
                continue;
            }

            // Two-character operators/symbols
            if c == '=' {
                self.next_char();
                let token = if self.chars.peek() == Some(&'=') {
                    self.next_char();
                    Token::DoubleEq
                } else {
                    Token::Eq
                };
                let span = Span::new(start_line, start_col);
                tokens.push(TokenWithSpan { token, span });
                continue;
            }

            if c == '-' {
                self.next_char();
                let token = if self.chars.peek() == Some(&'>') {
                    self.next_char();
                    Token::Arrow
                } else {
                    Token::Minus
                };
                let span = Span::new(start_line, start_col);
                tokens.push(TokenWithSpan { token, span });
                continue;
            }

            if c == '>' {
                self.next_char();
                let token = if self.chars.peek() == Some(&'=') {
                    self.next_char();
                    Token::Gte
                } else {
                    Token::Gt
                };
                let span = Span::new(start_line, start_col);
                tokens.push(TokenWithSpan { token, span });
                continue;
            }

            if c == '<' {
                self.next_char();
                let token = if self.chars.peek() == Some(&'=') {
                    self.next_char();
                    Token::Lte
                } else {
                    Token::Lt
                };
                let span = Span::new(start_line, start_col);
                tokens.push(TokenWithSpan { token, span });
                continue;
            }

            if c == '.' {
                self.next_char();
                let token = if self.chars.peek() == Some(&'.') {
                    self.next_char();
                    Token::DotDot
                } else {
                    Token::Dot
                };
                let span = Span::new(start_line, start_col);
                tokens.push(TokenWithSpan { token, span });
                continue;
            }

            if c == ':' {
                self.next_char();
                let token = if self.chars.peek() == Some(&':') {
                    self.next_char();
                    Token::DoubleColon
                } else {
                    Token::Colon
                };
                let span = Span::new(start_line, start_col);
                tokens.push(TokenWithSpan { token, span });
                continue;
            }

            if c == '&' {
                self.next_char();
                if self.chars.peek() == Some(&'&') {
                    self.next_char();
                    let span = Span::new(start_line, start_col);
                    tokens.push(TokenWithSpan { token: Token::And, span });
                    continue;
                } else {
                    return Err(format!("Unexpected character: '&' (expected '&&') at line {}, col {}", start_line, start_col));
                }
            }

            if c == '|' {
                self.next_char();
                if self.chars.peek() == Some(&'|') {
                    self.next_char();
                    let span = Span::new(start_line, start_col);
                    tokens.push(TokenWithSpan { token: Token::Or, span });
                    continue;
                } else {
                    return Err(format!("Unexpected character: '|' (expected '||') at line {}, col {}", start_line, start_col));
                }
            }

            if c == '!' {
                self.next_char();
                let token = if self.chars.peek() == Some(&'=') {
                    self.next_char();
                    Token::NotEq
                } else {
                    Token::Not
                };
                let span = Span::new(start_line, start_col);
                tokens.push(TokenWithSpan { token, span });
                continue;
            }

            // Single-character symbols
            let tok = match c {
                '+' => Token::Plus,
                '*' => Token::Mul,
                '(' => Token::LParen,
                ')' => Token::RParen,
                '{' => Token::LBrace,
                '}' => Token::RBrace,
                '[' => Token::LBracket,
                ']' => Token::RBracket,
                ',' => Token::Comma,
                ';' => Token::Semicolon,
                _ => return Err(format!("Unexpected character: '{}' at line {}, col {}", c, start_line, start_col)),
            };
            self.next_char();
            let span = Span::new(start_line, start_col);
            tokens.push(TokenWithSpan { token: tok, span });
        }
        Ok(tokens)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lexer_basic() {
        let input = "module Test; type A = { x: Field } circuit main() -> bool { return true; }";
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens[0].token, Token::Module);
        assert_eq!(tokens[1].token, Token::Ident("Test".to_string()));
        assert_eq!(tokens[2].token, Token::Semicolon);
        assert_eq!(tokens[3].token, Token::Type);
        assert_eq!(tokens[4].token, Token::Ident("A".to_string()));
        assert_eq!(tokens[5].token, Token::Eq);
        assert_eq!(tokens[6].token, Token::LBrace);
        assert_eq!(tokens[7].token, Token::Ident("x".to_string()));
        assert_eq!(tokens[8].token, Token::Colon);
        assert_eq!(tokens[9].token, Token::FieldTy);
        assert_eq!(tokens[10].token, Token::RBrace);
    }
}

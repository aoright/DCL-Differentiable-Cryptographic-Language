//! Lexical analysis for the DCL language.
//!
//! Converts raw source text into a sequence of [`TokenWithSpan`] values, handling
//! comments (both `//` line comments and `/* */` block comments), keywords,
//! identifiers, decimal and hexadecimal number literals, and all operators/delimiters.

use crate::ast::Span;

/// All token types recognized by the DCL lexer.
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

/// A token paired with its source location span.
#[derive(Debug, Clone, PartialEq)]
pub struct TokenWithSpan {
    pub token: Token,
    pub span: Span,
}

/// Lexer that produces a stream of tokens from DCL source code.
///
/// Supports:
/// - Line comments (`// ...`)
/// - Block comments (`/* ... */`), including nested
/// - Decimal number literals
/// - Hexadecimal number literals (`0x1A2B`)
/// - All keywords, operators, and delimiters defined in [`Token`]
pub struct Lexer<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
    line: usize,
    col: usize,
}

impl<'a> Lexer<'a> {
    /// Create a new lexer for the given source string.
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

    fn make_span(&self, start_line: usize, start_col: usize) -> Span {
        Span::range(start_line, start_col, self.line, self.col)
    }

    /// Tokenize the entire source input, returning a vector of tokens or an error.
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
                    // Line comment — skip to end of line
                    while let Some(nc) = self.next_char() {
                        if nc == '\n' {
                            break;
                        }
                    }
                    continue;
                } else if self.chars.peek() == Some(&'*') {
                    // Block comment — handle nesting
                    self.next_char(); // consume '*'
                    let mut depth = 1;
                    while depth > 0 {
                        match self.next_char() {
                            Some('/') if self.chars.peek() == Some(&'*') => {
                                self.next_char();
                                depth += 1;
                            }
                            Some('*') if self.chars.peek() == Some(&'/') => {
                                self.next_char();
                                depth -= 1;
                            }
                            None => {
                                return Err(format!(
                                    "[Error at line {}, col {}]: Unterminated block comment",
                                    start_line, start_col
                                ));
                            }
                            _ => {}
                        }
                    }
                    continue;
                } else {
                    let span = self.make_span(start_line, start_col);
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
                let span = self.make_span(start_line, start_col);
                tokens.push(TokenWithSpan { token, span });
                continue;
            }

            if c.is_numeric() {
                let mut num_str = String::new();

                // Check for hexadecimal literal (0x...)
                if c == '0' {
                    num_str.push(c);
                    self.next_char();
                    if self.chars.peek() == Some(&'x') || self.chars.peek() == Some(&'X') {
                        self.next_char(); // consume 'x'
                        let mut hex_str = String::new();
                        while let Some(&nc) = self.chars.peek() {
                            if nc.is_ascii_hexdigit() {
                                hex_str.push(nc);
                                self.next_char();
                            } else {
                                break;
                            }
                        }
                        if hex_str.is_empty() {
                            return Err(format!(
                                "[Error at line {}, col {}]: Invalid hexadecimal literal: 0x",
                                start_line, start_col
                            ));
                        }
                        // Convert hex to decimal string for uniform downstream processing
                        let decimal_val = u128::from_str_radix(&hex_str, 16).map_err(|_| {
                            format!(
                                "[Error at line {}, col {}]: Hexadecimal literal too large: 0x{}",
                                start_line, start_col, hex_str
                            )
                        })?;
                        let span = self.make_span(start_line, start_col);
                        tokens.push(TokenWithSpan { token: Token::Num(decimal_val.to_string()), span });
                        continue;
                    }
                    // Not hex — continue as decimal starting with '0'
                } else {
                    num_str.push(c);
                    self.next_char();
                }

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
                let span = self.make_span(start_line, start_col);
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
                let span = self.make_span(start_line, start_col);
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
                let span = self.make_span(start_line, start_col);
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
                let span = self.make_span(start_line, start_col);
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
                let span = self.make_span(start_line, start_col);
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
                let span = self.make_span(start_line, start_col);
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
                let span = self.make_span(start_line, start_col);
                tokens.push(TokenWithSpan { token, span });
                continue;
            }

            if c == '&' {
                self.next_char();
                if self.chars.peek() == Some(&'&') {
                    self.next_char();
                    let span = self.make_span(start_line, start_col);
                    tokens.push(TokenWithSpan { token: Token::And, span });
                    continue;
                } else {
                    return Err(format!("[Error at line {}, col {}]: Unexpected character: '&' (expected '&&')", start_line, start_col));
                }
            }

            if c == '|' {
                self.next_char();
                if self.chars.peek() == Some(&'|') {
                    self.next_char();
                    let span = self.make_span(start_line, start_col);
                    tokens.push(TokenWithSpan { token: Token::Or, span });
                    continue;
                } else {
                    return Err(format!("[Error at line {}, col {}]: Unexpected character: '|' (expected '||')", start_line, start_col));
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
                let span = self.make_span(start_line, start_col);
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
                _ => return Err(format!("[Error at line {}, col {}]: Unexpected character: '{}'", start_line, start_col, c)),
            };
            self.next_char();
            let span = self.make_span(start_line, start_col);
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

    #[test]
    fn test_block_comment() {
        let input = "module Test /* this is a comment */ circuit main() -> bool { return true; }";
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens[0].token, Token::Module);
        assert_eq!(tokens[1].token, Token::Ident("Test".to_string()));
        assert_eq!(tokens[2].token, Token::Circuit);
    }

    #[test]
    fn test_nested_block_comment() {
        let input = "module Test /* outer /* inner */ still comment */ circuit main() -> bool { return true; }";
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens[0].token, Token::Module);
        assert_eq!(tokens[1].token, Token::Ident("Test".to_string()));
        assert_eq!(tokens[2].token, Token::Circuit);
    }

    #[test]
    fn test_unterminated_block_comment() {
        let input = "module Test /* unterminated";
        let mut lexer = Lexer::new(input);
        let result = lexer.tokenize();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unterminated block comment"));
    }

    #[test]
    fn test_hex_literal() {
        let input = "module Test circuit main() -> Field { return 0xFF; }";
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        // 0xFF should be converted to "255"
        let num_token = tokens.iter().find(|t| matches!(&t.token, Token::Num(_))).unwrap();
        assert_eq!(num_token.token, Token::Num("255".to_string()));
    }

    #[test]
    fn test_span_range() {
        let input = "module Test";
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        // "module" starts at col 1
        assert_eq!(tokens[0].span.start_line, 1);
        assert_eq!(tokens[0].span.start_col, 1);
    }
}

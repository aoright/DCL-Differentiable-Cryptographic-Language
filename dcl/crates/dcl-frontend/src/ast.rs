/// Abstract Syntax Tree definitions for the DCL (Differentiable Cryptographic Language).
///
/// This module defines the core data structures used to represent parsed DCL programs,
/// including types, expressions, statements, and top-level declarations.
use serde::{Serialize, Deserialize};

/// Source location span tracking both start and end positions.
///
/// Used for precise error reporting and IDE integration (e.g., LSP hover ranges).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub start_line: usize,
    pub start_col: usize,
    pub end_line: usize,
    pub end_col: usize,
}

impl Span {
    /// Create a span that covers a single point (start == end).
    pub fn new(line: usize, col: usize) -> Self {
        Self { start_line: line, start_col: col, end_line: line, end_col: col }
    }

    /// Create a span with explicit start and end.
    pub fn range(start_line: usize, start_col: usize, end_line: usize, end_col: usize) -> Self {
        Self { start_line, start_col, end_line, end_col }
    }

    /// Merge two spans into one that covers both (union).
    pub fn merge(&self, other: &Span) -> Span {
        Span {
            start_line: self.start_line.min(other.start_line),
            start_col: if self.start_line < other.start_line { self.start_col }
                       else if other.start_line < self.start_line { other.start_col }
                       else { self.start_col.min(other.start_col) },
            end_line: self.end_line.max(other.end_line),
            end_col: if self.end_line > other.end_line { self.end_col }
                     else if other.end_line > self.end_line { other.end_col }
                     else { self.end_col.max(other.end_col) },
        }
    }

    /// Legacy accessor for backward compatibility.
    pub fn line(&self) -> usize { self.start_line }
    /// Legacy accessor for backward compatibility.
    pub fn col(&self) -> usize { self.start_col }
}

/// Primitive and composite types in DCL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Type {
    /// Prime field element (BN254).
    Field,
    /// Boolean value.
    Bool,
    /// Constrained unsigned integer type with explicit bit width.
    /// Compiles down to Field + range_check(bits) at the IR level.
    /// Supported widths: 8, 16, 32, 64.
    Uint(usize),
    /// User-defined struct type, referenced by name.
    Struct(String),
    /// Fixed-size homogeneous array.
    Array(Box<Type>, usize),
}

/// Parameter visibility modifiers for ZKP witness classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Visibility {
    /// Private witness input (not revealed to verifier).
    Private,
    /// Public input (known to both prover and verifier).
    Public,
    /// Shared input (known to multiple provers in MPC setting).
    Shared,
}

/// Struct type definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructDef {
    pub name: String,
    pub fields: Vec<(String, Type)>,
    pub span: Span,
}

/// Unary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnOp {
    /// Logical NOT (`!x`).
    Not,
    /// Arithmetic negation (`-x`), lowered as `0 - x`.
    Neg,
}

/// Binary operators with ZK-circuit semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    NotEq,
    Gte,
    Lte,
    Lt,
    Gt,
    And,
    Or,
}

/// Expressions in the DCL language.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    /// Variable reference.
    Var(String, Span),
    /// Field constant literal (arbitrary precision decimal or hex).
    ConstField(String, Span),
    /// Boolean constant literal.
    ConstBool(bool, Span),
    /// Unary operation.
    Unary(UnOp, Box<Expr>, Span),
    /// Binary operation.
    Binary(BinOp, Box<Expr>, Box<Expr>, Span),
    /// Function / circuit call.
    Call(String, Vec<Expr>, Span),
    /// Struct field access (`expr.field`).
    Access(Box<Expr>, String, Span),
    /// Array index access (`expr[index]`).
    Index(Box<Expr>, Box<Expr>, Span),
}

impl Expr {
    /// Returns the source span for this expression.
    pub fn span(&self) -> Span {
        match self {
            Expr::Var(_, s) => *s,
            Expr::ConstField(_, s) => *s,
            Expr::ConstBool(_, s) => *s,
            Expr::Unary(_, _, s) => *s,
            Expr::Binary(_, _, _, s) => *s,
            Expr::Call(_, _, s) => *s,
            Expr::Access(_, _, s) => *s,
            Expr::Index(_, _, s) => *s,
        }
    }
}

/// Statements in circuit bodies.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Stmt {
    /// Variable binding: `let [mut] name [: Type] = expr;`
    Let(String, bool, Option<Type>, Expr, Span),
    /// Assertion: `assert expr;`
    Assert(Expr, Span),
    /// Assignment: `lhs = rhs;`
    Assign(Box<Expr>, Expr, Span),
    /// Return value: `return expr;`
    Return(Expr, Span),
    /// Bounded loop: `for var in start..end { body }`
    For(String, Box<Expr>, Box<Expr>, Vec<Stmt>, Span),
    /// Conditional branch: `if cond { then } [else { otherwise }]`
    If(Box<Expr>, Vec<Stmt>, Option<Vec<Stmt>>, Span),
    /// Expression statement (for side-effecting calls): `expr;`
    ExprStmt(Expr, Span),
}

impl Stmt {
    /// Returns the source span for this statement.
    pub fn span(&self) -> Span {
        match self {
            Stmt::Let(_, _, _, _, s) => *s,
            Stmt::Assert(_, s) => *s,
            Stmt::Assign(_, _, s) => *s,
            Stmt::Return(_, s) => *s,
            Stmt::For(_, _, _, _, s) => *s,
            Stmt::If(_, _, _, s) => *s,
            Stmt::ExprStmt(_, s) => *s,
        }
    }
}

/// Circuit parameter declaration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    pub visibility: Visibility,
    pub ty: Type,
}

/// A circuit (function) definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Circuit {
    pub name: String,
    pub params: Vec<Parameter>,
    pub return_ty: Type,
    pub body: Vec<Stmt>,
    pub is_extern: bool,
    pub span: Span,
}

/// Top-level module containing types, imports, and circuits.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Module {
    pub name: String,
    pub imports: Vec<Vec<String>>,
    pub types: Vec<StructDef>,
    pub circuits: Vec<Circuit>,
}

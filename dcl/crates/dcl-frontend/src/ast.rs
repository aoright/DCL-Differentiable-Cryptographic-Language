use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub line: usize,
    pub col: usize,
}

impl Span {
    pub fn new(line: usize, col: usize) -> Self {
        Self { line, col }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Type {
    Field,
    Bool,
    Struct(String),
    Array(Box<Type>, usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Visibility {
    Private,
    Public,
    Shared,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructDef {
    pub name: String,
    pub fields: Vec<(String, Type)>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnOp {
    Not,
}

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    Var(String, Span),
    ConstField(String, Span),
    ConstBool(bool, Span),
    Unary(UnOp, Box<Expr>, Span),
    Binary(BinOp, Box<Expr>, Box<Expr>, Span),
    Call(String, Vec<Expr>, Span),
    Access(Box<Expr>, String, Span),
    Index(Box<Expr>, Box<Expr>, Span),
}

impl Expr {
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Stmt {
    Let(String, bool, Option<Type>, Expr, Span),
    Assert(Expr, Span),
    Assign(Box<Expr>, Expr, Span),
    Return(Expr, Span),
    For(String, Box<Expr>, Box<Expr>, Vec<Stmt>, Span),
    If(Box<Expr>, Vec<Stmt>, Option<Vec<Stmt>>, Span),
}

impl Stmt {
    pub fn span(&self) -> Span {
        match self {
            Stmt::Let(_, _, _, _, s) => *s,
            Stmt::Assert(_, s) => *s,
            Stmt::Assign(_, _, s) => *s,
            Stmt::Return(_, s) => *s,
            Stmt::For(_, _, _, _, s) => *s,
            Stmt::If(_, _, _, s) => *s,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    pub visibility: Visibility,
    pub ty: Type,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Circuit {
    pub name: String,
    pub params: Vec<Parameter>,
    pub return_ty: Type,
    pub body: Vec<Stmt>,
    pub is_extern: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Module {
    pub name: String,
    pub imports: Vec<Vec<String>>,
    pub types: Vec<StructDef>,
    pub circuits: Vec<Circuit>,
}

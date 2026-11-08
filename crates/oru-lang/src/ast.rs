//! `.oru` 的语法树与共享语义节点。

use crate::span::Span;

#[derive(Debug, Clone)]
pub struct Program {
    pub items: Vec<Item>,
}

#[derive(Debug, Clone)]
pub enum Item {
    Game { name: String, span: Span },
    Seats { count: u32, span: Span },
    Edition { edition: u32, span: Span },
    Resource(Resource),
    Action(Action),
    Trigger(Trigger),
    Win(WinRule),
}

#[derive(Debug, Clone)]
pub struct Resource {
    pub name: String,
    pub name_span: Span,
    pub span: Span,
    pub per: Per,
    pub per_span: Option<Span>,
    pub init: Option<i64>,
    pub min: Option<i64>,
    pub max: Option<i64>,
    pub visibility: Visibility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Per {
    Player,
    Game,
    Team,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Public,
    Private,
}

#[derive(Debug, Clone)]
pub struct Action {
    pub id: String,
    pub id_span: Span,
    pub label: String,
    pub span: Span,
    pub when: Option<Expr>,
    pub effects: Vec<Effect>,
}

#[derive(Debug, Clone)]
pub struct Trigger {
    pub on: String,
    pub on_span: Span,
    pub span: Span,
    pub effects: Vec<Effect>,
}

#[derive(Debug, Clone)]
pub struct WinRule {
    pub when: Expr,
    pub say: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Win,
    Lose,
    Draw,
}

#[derive(Debug, Clone)]
pub enum EffectKind {
    Set {
        path: Path,
        value: Expr,
    },
    Add {
        path: Path,
        value: Expr,
    },
    Sub {
        path: Path,
        value: Expr,
    },
    Say {
        text: String,
    },
    End {
        outcome: Outcome,
        text: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub struct Effect {
    pub kind: EffectKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    Int(i64),
    Bool(bool),
    Path(Path),
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Not,
    Neg,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathRoot {
    SelfSeat,
    Other,
    Game,
}

#[derive(Debug, Clone)]
pub struct Path {
    pub root: PathRoot,
    pub name: String,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Type {
    Int,
    Bool,
    Unknown,
}

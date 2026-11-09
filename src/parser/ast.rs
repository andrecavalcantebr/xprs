//! AST de um módulo XPR (ver `plano-parser.md`, seção "Estruturas de dados").
//!
//! O parser produz **apenas** a estrutura sintática: nada de tipos, resolução de nomes ou
//! distinção lista/tupla (essa distinção depende de unificação de tipos e pertence ao
//! Codegen/Sema). Decisões relevantes já fixadas no plano:
//!
//! * **Não existe nó `Paren`**: parênteses só guiam precedência; a forma da árvore já representa o
//!   agrupamento.
//! * `v[a, b, c]` e `v[a][b][c]` produzem o **mesmo** nó `Indexed` (base + lista de índices); a
//!   expansão para indexação sucessiva é decisão do lowering, não do parser.
//! * Literais entre colchetes são `BracketLiteral` (lista × tupla fica para depois).
//! * Strings chegam aqui já **decodificadas** (os escapes `\n \t \r \0 \" \' \\` são resolvidos no
//!   parser, por ser uma operação léxica/sintática que não depende de tipos).

use crate::lexer::Span;

// -------------------------------------------------------------------------------------------
// Estrutura do módulo
// -------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct Module {
    pub imports: Vec<ImportDecl>,
    pub definitions: Vec<Definition>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportDecl {
    /// Nome do módulo importado (conteúdo da string, sem a extensão `.x`).
    pub name: String,
    /// Alias local; quando ausente na fonte, é igual a `name`.
    pub alias: Option<String>,
    pub span: Span,
}

impl ImportDecl {
    /// Alias efetivo (o explícito ou, na falta dele, o próprio nome do módulo).
    pub fn effective_alias(&self) -> &str {
        self.alias.as_deref().unwrap_or(&self.name)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Definition {
    pub name: String,
    pub initializer: Initializer,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Initializer {
    FnDef(FnDef),
    /// Já validado sintaticamente: não contém `fn_call`, `block`, `iif`, `for`, atribuição nem
    /// `fn_def` (subetapa I de `plano-parser.md`).
    ConstExpr(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct FnDef {
    pub param: ParamPattern,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    /// O valor do bloco é o do último statement.
    pub statements: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Import(ImportDecl),
    Expr(Expr),
}

// -------------------------------------------------------------------------------------------
// Expressões
// -------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    IntLit(String, Span),
    RealLit(String, Span),
    /// String já decodificada e o delimitador original (`"` ou `'`).
    StringLit(String, char, Span),
    QualifiedName {
        module_alias: Option<String>,
        name: String,
        span: Span,
    },

    Assignment {
        target: BindingPattern,
        value: Box<Expr>,
        span: Span,
    },
    FnDef(Box<FnDef>),

    /// Operandos de um `or` encadeado (curto-circuito é decisão de sema/codegen).
    LogicalOr(Vec<Expr>),
    LogicalAnd(Vec<Expr>),
    LogicalNot(Box<Expr>, Span),

    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        span: Span,
    },
    Unary {
        op: UnOp,
        operand: Box<Expr>,
        span: Span,
        /// O sinal estava diretamente colado ao operando na fonte (`Token::adjacent_to_next`).
        sign_adjacent: bool,
    },

    Indexed {
        base: Box<Expr>,
        indices: Vec<Expr>,
        span: Span,
    },
    FnCall {
        target: Box<Expr>,
        arg: Option<Box<Expr>>,
        span: Span,
    },

    Block(Block),
    Iif {
        cond: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Box<Expr>,
        span: Span,
    },
    For {
        init: Box<Expr>,
        cond: Box<Expr>,
        step: Box<Expr>,
        body: Box<Expr>,
        span: Span,
    },
    Try(Box<Expr>, Span),

    /// `[e1, e2, ...]` — lista **ou** tupla; a distinção é semântica.
    BracketLiteral(Vec<Expr>, Span),
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::IntLit(_, s)
            | Expr::RealLit(_, s)
            | Expr::StringLit(_, _, s)
            | Expr::LogicalNot(_, s)
            | Expr::Try(_, s) => *s,
            Expr::QualifiedName { span, .. }
            | Expr::Assignment { span, .. }
            | Expr::Binary { span, .. }
            | Expr::Unary { span, .. }
            | Expr::Indexed { span, .. }
            | Expr::FnCall { span, .. }
            | Expr::Iif { span, .. }
            | Expr::For { span, .. }
            | Expr::BracketLiteral(_, span) => *span,
            Expr::FnDef(f) => f.span,
            Expr::Block(b) => b.span,
            Expr::LogicalOr(operands) | Expr::LogicalAnd(operands) => {
                // Um `or`/`and` encadeado sempre tem ao menos 2 operandos (o parser só cria o nó
                // quando encontra o operador), então o span vai do primeiro ao último operando.
                let first = operands.first().expect("Or/And sem operandos").span();
                let last = operands.last().expect("Or/And sem operandos").span();
                Span {
                    line_start: first.line_start,
                    col_start: first.col_start,
                    line_end: last.line_end,
                    col_end: last.col_end,
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Lt,
    Gt,
    Le,
    Ge,
    EqEq,
    NotEq,
}

impl BinOp {
    /// Símbolo de superfície, como aparece no dump da AST.
    pub fn symbol(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Mod => "%",
            BinOp::Lt => "<",
            BinOp::Gt => ">",
            BinOp::Le => "<=",
            BinOp::Ge => ">=",
            BinOp::EqEq => "==",
            BinOp::NotEq => "!=",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Plus,
    Minus,
}

impl UnOp {
    pub fn symbol(self) -> &'static str {
        match self {
            UnOp::Plus => "+",
            UnOp::Minus => "-",
        }
    }
}

// -------------------------------------------------------------------------------------------
// Padrões e anotações de tipo
// -------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum BindingPattern {
    Name(String, Span),
    List(Vec<BindingPattern>, Span),
}

impl BindingPattern {
    pub fn span(&self) -> Span {
        match self {
            BindingPattern::Name(_, s) => *s,
            BindingPattern::List(_, s) => *s,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParamPattern {
    Name {
        name: String,
        ty: Option<TypeAnnotation>,
        span: Span,
    },
    List(Vec<ParamPattern>, Span),
}

impl ParamPattern {
    pub fn span(&self) -> Span {
        match self {
            ParamPattern::Name { span, .. } => *span,
            ParamPattern::List(_, span) => *span,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeAnnotation {
    Bool,
    Int,
    Real,
    String,
    Fn,
    /// `[]` — qualquer lista.
    AnyList,
    /// `[T]` — lista de `T`.
    ListOf(Box<TypeAnnotation>),
    /// `[T1, T2, ...]` — tupla.
    Tuple(Vec<TypeAnnotation>),
}

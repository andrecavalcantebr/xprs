# Plano — Etapa Parser

Fonte normativa: `xpr-specs.md`, Seção 3 (EBNF completa), e as notas de "Sinais numéricos
unários e espaços em branco" / "Ordem de avaliação" em Seção 4 (pré-ambulo). Depende de
`plano-lexer.md` (tokens com `Span` e `adjacent_to_next`).

## Objetivo

Construir a AST de um módulo, aplicando **apenas** regras estruturais e de precedência — nenhuma
checagem de tipo, nenhuma resolução de nomes, nenhuma distinção lista/tupla (essa distinção
depende de tipos e é responsabilidade da etapa Codegen/Sema; ver `plano-codegen.md` subetapa C).
O parser deve, no entanto, aplicar as restrições puramente **sintáticas** da gramática: posição
de `import`, forma de `const_expr` no nível de módulo, alvos de atribuição válidos, `;` obrigatório
em blocos, e a regra de adjacência de sinal unário.

## Dependências

Nenhuma crate externa necessária para a árvore em si. (Avaliar `thiserror` ou equivalente só se o
agente executor achar que reduz boilerplate de erro — não é um requisito da spec.)

## Estruturas de dados (esboço de API)

```rust
// src/parser/ast.rs

pub struct Module {
    pub imports: Vec<ImportDecl>,
    pub definitions: Vec<Definition>,
}

pub struct ImportDecl {
    pub name: String,            // literal de string, sem extensão
    pub alias: Option<String>,   // default = name
    pub span: Span,
}

pub struct Definition {
    pub name: String,
    pub initializer: Initializer,
    pub span: Span,
}

pub enum Initializer {
    FnDef(FnDef),
    ConstExpr(Expr), // parser já valida a forma restrita (ver subetapa E)
}

pub struct FnDef {
    pub param: ParamPattern,
    pub body: Block,
    pub span: Span,
}

pub struct Block {
    pub statements: Vec<Stmt>, // último statement determina o valor do bloco
    pub span: Span,
}

pub enum Stmt {
    Import(ImportDecl),  // só válido como um dos primeiros do bloco MAIS EXTERNO do programa
    Expr(Expr),
}

pub enum Expr {
    IntLit(String, Span),
    RealLit(String, Span),
    StringLit(String, char, Span), // cru; decodificação de escapes: ver nota abaixo
    QualifiedName { module_alias: Option<String>, name: String, span: Span },

    Assignment { target: BindingPattern, value: Box<Expr>, span: Span },
    FnDef(Box<FnDef>),

    LogicalOr(Vec<Expr>),   // operandos de "or" encadeado (curto-circuito fica para sema/codegen)
    LogicalAnd(Vec<Expr>),
    LogicalNot(Box<Expr>, Span),

    Binary { op: BinOp, lhs: Box<Expr>, rhs: Box<Expr>, span: Span },
    Unary { op: UnOp, operand: Box<Expr>, span: Span, sign_adjacent: bool },

    Indexed { base: Box<Expr>, indices: Vec<Expr>, span: Span }, // v[a,b,c] vira indices=[a,b,c]
    FnCall { target: Box<Expr>, arg: Option<Box<Expr>>, span: Span },

    Block(Block),
    Iif { cond: Box<Expr>, then_branch: Box<Expr>, else_branch: Box<Expr>, span: Span },
    For { init: Box<Expr>, cond: Box<Expr>, step: Box<Expr>, body: Box<Expr>, span: Span },
    Try(Box<Expr>, Span),

    /// Literal entre colchetes "[e1, e2, ...]". Lista x tupla NÃO é decidido aqui
    /// (depende de unificação de tipos) — ver plano-codegen.md subetapa C.
    BracketLiteral(Vec<Expr>, Span),

    Paren(Box<Expr>), // mantido só se útil para diagnósticos; pode ser eliminado no lowering
}

pub enum BinOp { Add, Sub, Mul, Div, Mod, Lt, Gt, Le, Ge, EqEq, NotEq }
pub enum UnOp { Plus, Minus }

pub enum BindingPattern {
    Name(String, Span),
    List(Vec<BindingPattern>, Span),
}

pub enum ParamPattern {
    Name { name: String, ty: Option<TypeAnnotation>, span: Span },
    List(Vec<ParamPattern>, Span),
}

pub enum TypeAnnotation {
    Bool, Int, Real, String, Fn,
    AnyList,                       // "[]"
    ListOf(Box<TypeAnnotation>),   // "[T]"
    Tuple(Vec<TypeAnnotation>),    // "[T1, T2, ...]"
}

// src/parser/mod.rs
pub struct Parser<'t> { /* slice de tokens + cursor */ }

#[derive(Debug)]
pub struct SyntaxError { pub message: String, pub span: Span }

impl<'t> Parser<'t> {
    pub fn new(tokens: &'t [Token]) -> Self;
    pub fn parse_module(&mut self) -> Result<Module, SyntaxError>;
}
```

**Nota sobre strings:** decidir explicitamente em qual etapa os escapes (`\n`, `\t`, `\r`, `\0`,
`\"`, `\'`, `\\`) são decodificados — parser (ao construir `StringLit`) ou sema. Recomendação:
decodificar no parser, pois é puramente sintático/léxico (não depende de tipos) e simplifica a
etapa Codegen. Registrar a escolha final no `RELATORIO_PROGRESSO.md` da etapa.

## Subetapas (TDD)

**A. Infra do parser** — cursor sobre tokens, `peek`/`advance`/`expect`, `SyntaxError` com `Span`,
parada no primeiro erro (Seção 7: "A compilação é interrompida no primeiro erro").

**B. Cadeia de precedência de expressões** — `logical_or → logical_and → logical_not → equality →
relational → arithmetic → term → factor → indexed → primary`, respeitando associatividade à
esquerda (exceto atribuição, à direita) e avaliação esquerda→direita.

**C. Primary**: literais, `qualified_name` (`alias.nome`), `fn_call`, `block`, `iif_expr`,
`for_expr`, `list_expr` (= `BracketLiteral`), `try_expr`, `"(" expression ")"`.

**D. `indexed`**: colchetes múltiplos encadeados após `primary`, incluindo o açúcar
`v[a, b, c]` → `indices = [a, b, c]` (equivalente a `v[a][b][c]`, mas representado em um único nó
`Indexed` com lista de índices — a expansão para indexação sucessiva é decisão do lowering em
Codegen, não do parser).

**E. Padrões**: `binding_pattern`, `list_pattern`, `param_pattern` com anotação `:tipo` opcional,
e o `type` da gramática (nomes de tipo reconhecidos só após `:`, nunca palavras reservadas).

**F. Atribuição e `fn_def`**: `assignment` associativa à direita; validação sintática de que o
alvo de atribuição é um `binding_pattern` válido (identificador ou padrão de lista recursivo —
nunca uma expressão indexada, conforme Seção 7 "Expressões indexadas nunca são alvos de
atribuição válidos").

**G. Blocos e `;` obrigatório**: `block = "{" { statement } "}"`; cada `statement` exige `;`
final; bloco vazio é válido.

**H. Regra de adjacência de sinal unário** (usa `Token.adjacent_to_next` do lexer): `2 * -3` e
`2 *-3` válidos; `2 *- 3`, `- 3`, `+ 3` no início de uma expressão unária são `syntax-error`. Testar
contra os exemplos literais da Seção 4 (pré-ambulo "Sinais numéricos unários...").

**I. Nível de módulo**: `import_decl` só antes da primeira `definition`; `import` como `statement`
só nos primeiros statements do bloco mais externo do **programa** (não de blocos internos
arbitrários); forma de `definition` (`identifier "=" initializer`); validação estrutural de
`const_expr` — rejeitar (como `syntax-error` sintático, antes de qualquer checagem de tipo)
`fn_call`, `block`, `iif_expr`, `for_expr`, atribuição ou `fn_def` aninhados dentro de um
`const_expr` usado como inicializador direto de nível de módulo (quando o inicializador não é ele
mesmo um `fn_def` de nível superior).

**J. Rejeição de programa vazio**: "código fonte completo que não contenha nenhuma expressão" é
erro (Seção 7).

## Casos adversos obrigatórios (derivados da Seção 7 "Condições de erro obrigatórias")

* Sintaxe inválida (geral).
* Alvo de atribuição inválido (expressão indexada como alvo).
* Arquivo sem nenhuma definição/expressão.
* `import` fora da posição permitida.
* `y = f(a, b)` — chamada com dois argumentos posicionais é erro de sintaxe (funções XPR têm um
  único argumento; ver 4.5). Testar explicitamente esse exemplo da spec.

## Critério de conclusão da etapa

Toda a gramática da Seção 3 coberta por testes (`cargo test --test parser_tests`), incluindo os
exemplos literais citados ao longo da Seção 4 (ex.: `fat = fn(x){ iif(x<=1, 1, x*fat(x-1)); };`,
`a = b = fn(x){ iif(x==0, 0, a(x-1)); };`) parseados com sucesso em AST, e os casos adversos acima
rejeitados com `SyntaxError` na posição correta.

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
}
// Decisão: "(" expression ")" NÃO produz um nó próprio. Os parênteses só servem para
// forçar a precedência durante o parsing; a árvore resultante já representa o agrupamento
// pela sua própria forma, então um nó `Paren` seria ruído sem valor semântico (e inflaria
// à toa os dumps de AST usados como oráculo de teste).

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxErrorKind {
    UnexpectedToken,
    MissingSemicolon,
    InvalidAssignmentTarget,   // alvo de atribuição não é identificador/padrão de lista
    MultiArgCall,              // "f(a, b)" — funções XPR têm um único argumento (4.5)
    ImportNotAtModuleStart,    // import fora da posição permitida (4.1)
    InvalidConstExpr,          // fn_call/block/iif/for/atribuição/fn_def dentro de const_expr de topo
    SignNotAdjacent,           // "+"/"-" unário separado do literal por espaço/comentário
    EmptyProgram,              // código fonte sem nenhuma definição/expressão
}

#[derive(Debug)]
pub struct SyntaxError { pub kind: SyntaxErrorKind, pub span: Span, pub detail: String }
// `detail` é texto livre para humanos (mensagem de diagnóstico); os testes golden-file
// comparam apenas `kind` + `span`, nunca `detail` — a mensagem pode evoluir livremente.

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

## Fixtures golden-file: exemplos `.x` e seus dumps de AST esperados

Mesma lógica do lexer (ver `plano-lexer.md`): `tests/parser/cases/*.x` + `*.ast` para sucesso,
`tests/parser/cases/errors/*.x` + `*.error` para rejeição. O critério de conclusão da etapa é
estes fixtures passando a 100%, não só os testes unitários por subetapa.

### Formato do dump de AST (`.ast`)

Uma árvore em texto, indentada **2 espaços por nível**, um nó por linha, sem parênteses de
fechamento. Cada linha é `<tag> [payload inline]`; os filhos de um nó vêm nas linhas seguintes,
indentados um nível a mais. **Spans não aparecem neste dump** — span já é testado
exaustivamente nos fixtures do lexer, e repetir span em cada nó da AST tornaria estes arquivos
grandes e frágeis sem agregar cobertura nova; a subetapa A exige, à parte, 1-2 testes unitários
(não golden-file) que verificam explicitamente que `Definition.span`/`FnDef.span`/etc. propagam a
posição correta.

Convenções de tag, por tipo de nó (`Expr`/`Stmt`/módulo):

| Nó | Linha | Filhos |
| --- | --- | --- |
| `Module` | `Module` | `Import ...` (sem filhos) e `Def <nome>` (1 filho: o inicializador), em ordem |
| `Import` | `Import name=<nome> alias=<alias>` | nenhum |
| `FnDef` | `FnDef param=<pattern>` | 1 filho: o `Block` do corpo |
| `Block` | `Block` | N filhos, um por `Stmt`, em ordem (um `Stmt::Import` vira uma linha `Import ...` sem filhos, igual ao de módulo) |
| `IntLit`/`RealLit` | `IntLit <texto bruto>` / `RealLit <texto bruto>` | nenhum |
| `StringLit` | `StrLit <string decodificada em `{:?}` do Rust>` | nenhum |
| `QualifiedName` | `Name <nome>` ou `Name <alias>.<nome>` | nenhum |
| `Assignment` | `Assign target=<pattern>` | 1 filho (sem rótulo): o valor |
| `LogicalOr`/`LogicalAnd` | `Or` / `And` | N filhos (sem rótulo), um por operando, em ordem |
| `LogicalNot` | `Not` | 1 filho (sem rótulo): o operando |
| `Binary` | `BinOp <símbolo>` (`+ - * / % < > <= >= == !=`) | 2 filhos rotulados: `lhs: <nó>` e `rhs: <nó>` |
| `Unary` | `Unary <símbolo> adj=<true|false>` | 1 filho (sem rótulo): o operando |
| `Indexed` | `Indexed` | 1 filho rotulado `base: <nó>`, depois um `idx[i]: <nó>` por índice, em ordem |
| `FnCall` | `Call` | `target: <nó>` sempre; `arg: <nó>` só se houver argumento |
| `Block` (como `Expr::Block`) | igual à entrada `Block` acima | — |
| `Iif` | `Iif` | `cond: <nó>`, `then: <nó>`, `else: <nó>` |
| `For` | `For` | `init: <nó>`, `cond: <nó>`, `step: <nó>`, `body: <nó>` |
| `Try` | `Try` | 1 filho (sem rótulo): a expressão |
| `BracketLiteral` | `Bracket` | N filhos (sem rótulo), um por elemento, em ordem |

Quando um filho rotulado (`lhs:`, `cond:`, `base:`, `idx[0]:`, ...) é por sua vez um nó composto,
a tag desse nó fica **na mesma linha**, após o rótulo (ex.: `cond: BinOp >`), e os filhos *dele*
vêm indentados um nível além dessa linha — não dois.

`<pattern>` (usado em `param=`/`target=`) é a forma de superfície da gramática, recursiva:
`x`, `x:int`, `[a, b]`, `[[a,b],[c,d]]`, `[a:int, b:int]`.

### Formato do erro (`.error`, uma única linha)

```
{line}:{col_start}-{col_end} {VarianteDeSyntaxErrorKind}
```
(mesma convenção de posição do lexer — linha dupla `L1:C1-L2:C2` só se o erro cruzar linhas, o
que não deve acontecer para nenhum destes casos).

### Exemplos incluídos (seed — a suíte deve crescer, nunca encolher)

| Arquivo | Cobre (subetapa) |
| --- | --- |
| `P01_precedencia_aritmetica.x` | B — precedência `+ - * /`, associatividade à esquerda |
| `P02_logica_curto_circuito.x` | B — precedência `or`/`and`/`not` |
| `P03_comparacoes.x` | B — `equality` envolvendo `relational` |
| `P04_unario_e_indexacao.x` | D — sinal unário sobre `indexed`, adjacência |
| `P05_indexacao_multipla.x` | D — açúcar `v[a, b]` como um único `Indexed` com 2 índices |
| `P06_funcao_e_chamada.x` | C — `fn_def`, `fn_call`, chamada dentro de corpo de função |
| `P07_bloco_e_atribuicao.x` | F/G — bloco com 2 statements, atribuição encadeada à direita |
| `P08_padroes_de_lista.x` | E — `param_pattern` com anotação `:int`, `binding_pattern` de lista aninhado |
| `P09_iif_e_for.x` | C — `iif_expr`, `for_expr` |
| `P10_try.x` | C — `try_expr` |
| `P11_modulo_import.x` | I — `import_decl` com e sem alias explícito, `qualified_name` como valor e como alvo de chamada |
| `errors/P_ERR1_dois_argumentos.x` | `y = f(a, b)` → `MultiArgCall` |
| `errors/P_ERR2_alvo_atribuicao_invalido.x` | `x[0] = 1` → `InvalidAssignmentTarget` |
| `errors/P_ERR3_import_fora_de_posicao.x` | `import` depois de uma definição → `ImportNotAtModuleStart` |
| `errors/P_ERR4_programa_vazio.x` | arquivo só com comentário → `EmptyProgram` |
| `errors/P_ERR5_const_expr_invalido.x` | `fn_call` direto num inicializador de topo → `InvalidConstExpr` |

Vale o mesmo aviso do `plano-lexer.md`: dumps derivados à mão, sem compilador disponível para
conferência mecânica nesta máquina. Formato é normativo; um fixture especificamente errado é
reportado e corrigido abertamente, nunca "ajustado" para calar um parser incorreto (AGENTS.md,
3.3.1).

## Script de execução

`scripts/run-parser-tests.sh` — mesma ideia do lexer: roda a suíte golden-file via `cargo test` e
imprime OK/NOK por caso, com status de saída refletindo 100% de sucesso ou não.

## Critério de conclusão da etapa

A etapa Parser só é considerada concluída quando **todos** os testes unitários por subetapa (A–J)
E **todos** os fixtures golden-file acima (`tests/parser/cases/`, sucesso e erro) passarem — ou
seja, quando `scripts/run-parser-tests.sh` sair com status 0. Isso inclui os exemplos literais
citados ao longo da Seção 4 (ex.: `fat = fn(x){ iif(x<=1, 1, x*fat(x-1)); };`,
`a = b = fn(x){ iif(x==0, 0, a(x-1)); };`) parseados com sucesso, e os casos adversos acima
rejeitados com o `SyntaxErrorKind` e a posição corretos. Sem aprovação parcial, e nenhum fixture é
alterado para mascarar um parser incorreto.

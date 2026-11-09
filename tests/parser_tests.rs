//! Testes unitários do Parser, organizados pelas subetapas A–J de `plano-parser.md`.
//!
//! A suíte golden-file equivalente (fixtures de `tests/parser/cases/`) vive em
//! `tests/parser_fixtures.rs`; estas duas suítes juntas fecham a etapa.
//!
//! Todos os helpers usam o **lexer real**, para que lexer e parser sejam exercitados juntos
//! exatamente como na pipeline de compilação.

use xprc::lexer::{Lexer, Span};
use xprc::parser::ast::*;
use xprc::parser::dump::dump_module;
use xprc::parser::{Parser, SyntaxErrorKind};

// -------------------------------------------------------------------------------------------
// Utilidades
// -------------------------------------------------------------------------------------------

fn tokens_of(source: &str) -> Vec<xprc::lexer::Token> {
    Lexer::new(source)
        .tokenize()
        .unwrap_or_else(|e| panic!("erro léxico em {:?}: {:?} {:?}", source, e.kind, e.span))
}

fn parse_ok(source: &str) -> Module {
    let tokens = tokens_of(source);
    Parser::new(&tokens).parse_module().unwrap_or_else(|e| {
        panic!(
            "erro sintático em {:?}: {:?} {:?} ({})",
            source, e.kind, e.span, e.detail
        )
    })
}

fn parse_err(source: &str) -> (SyntaxErrorKind, Span) {
    let tokens = tokens_of(source);
    match Parser::new(&tokens).parse_module() {
        Ok(module) => panic!(
            "esperava erro sintático em {:?}, mas o parse teve sucesso:\n{}",
            source,
            dump_module(&module)
        ),
        Err(e) => (e.kind, e.span),
    }
}

/// Dump de `<expr>` avaliada como statement de um bloco.
///
/// Envolver a expressão em `main = fn(x){ <expr>; };` é necessário porque no nível de módulo um
/// inicializador é `const_expr`, que **rejeita** chamadas, blocos, `iif`, `for` e atribuições
/// (subetapa I). Onde a expressão é const, use [`dump_const_expr`].
fn dump_expr(source_expr: &str) -> String {
    let module = parse_ok(&format!("main = fn(x){{ {}; }};", source_expr));
    let dump = dump_module(&module);
    let lines: Vec<&str> = dump.lines().collect();
    // `Module`, `  Def main`, `    FnDef param=x`, `      Block` e então o statement.
    let prefix = " ".repeat(8);
    lines[4..]
        .iter()
        .map(|line| {
            line.strip_prefix(prefix.as_str())
                .unwrap_or_else(|| panic!("linha sem indentação esperada: {:?}", line))
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Dump de `<expr>` como inicializador de nível de módulo (`const_expr`).
fn dump_const_expr(source_expr: &str) -> String {
    let module = parse_ok(&format!("r = {};", source_expr));
    let dump = dump_module(&module);
    let lines: Vec<&str> = dump.lines().collect();
    let prefix = " ".repeat(4);
    lines[2..]
        .iter()
        .map(|line| {
            line.strip_prefix(prefix.as_str())
                .unwrap_or_else(|| panic!("linha sem indentação esperada: {:?}", line))
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn span(line_start: u32, col_start: u32, line_end: u32, col_end: u32) -> Span {
    Span {
        line_start,
        col_start,
        line_end,
        col_end,
    }
}

fn param_of(source: &str) -> ParamPattern {
    let module = parse_ok(source);
    match &module.definitions[0].initializer {
        Initializer::FnDef(f) => f.param.clone(),
        other => panic!("esperava FnDef, veio {:?}", other),
    }
}

// -------------------------------------------------------------------------------------------
// Subetapa A — infraestrutura: cursor, erros com span, parada no primeiro erro
// -------------------------------------------------------------------------------------------

#[test]
fn subA_identificador_qualquer_pode_ser_definido() {
    let module = parse_ok("x = 1;");
    assert_eq!(module.definitions.len(), 1);
    assert_eq!(module.definitions[0].name, "x");
    assert!(module.imports.is_empty());
}

#[test]
fn subA_erro_de_sintaxe_traz_span_do_token_ofensor() {
    let (kind, bad_span) = parse_err("r = 1 + ;");
    assert_eq!(kind, SyntaxErrorKind::UnexpectedToken);
    // O `;` está na coluna 9.
    assert_eq!(bad_span, span(1, 9, 1, 10));
}

#[test]
fn subA_parse_para_no_primeiro_erro() {
    // Dois erros potenciais; só o primeiro (falta de `;` na linha 1) é reportado: o `b` da
    // linha 2 aparece onde se esperava `;`.
    let (kind, bad_span) = parse_err("a = 1\nb = 2;");
    assert_eq!(kind, SyntaxErrorKind::MissingSemicolon);
    assert_eq!(bad_span, span(2, 1, 2, 2));
}

#[test]
fn subA_spans_de_definicao_e_fndef_cobrem_o_texto() {
    // `f`(1) `=`(3) `fn`(5-6) `(`(7) `x`(8) `)`(9) `{`(10) `x`(12) `;`(13) `}`(15) `;`(16)
    let module = parse_ok("f = fn(x){ x; };");
    let def = &module.definitions[0];
    assert_eq!(def.span, span(1, 1, 1, 16));
    match &def.initializer {
        Initializer::FnDef(f) => {
            assert_eq!(f.span, span(1, 5, 1, 16));
            assert_eq!(f.body.span, span(1, 10, 1, 16));
            assert_eq!(f.param.span(), span(1, 8, 1, 9));
        }
        other => panic!("esperava FnDef, veio {:?}", other),
    }
}

#[test]
fn subA_aceita_varios_espacos_quebras_e_comentarios() {
    let module = parse_ok("// inicio\n\na /* c */ = /* c */ 1 ;\n");
    assert_eq!(module.definitions.len(), 1);
    assert_eq!(module.definitions[0].name, "a");
}

// -------------------------------------------------------------------------------------------
// Subetapa B — cadeia de precedência e associatividade
// -------------------------------------------------------------------------------------------

#[test]
fn subB_multiplicacao_antes_de_soma() {
    assert_eq!(
        dump_expr("1 + 2 * 3"),
        "BinOp +\n  lhs: IntLit 1\n  rhs: BinOp *\n    lhs: IntLit 2\n    rhs: IntLit 3"
    );
}

#[test]
fn subB_esquerda_para_direita_em_aritmetica() {
    assert_eq!(
        dump_expr("1 - 2 - 3"),
        "BinOp -\n  lhs: BinOp -\n    lhs: IntLit 1\n    rhs: IntLit 2\n  rhs: IntLit 3"
    );
}

#[test]
fn subB_relacional_antes_de_igualdade() {
    assert_eq!(
        dump_expr("1 < 2 == true"),
        "BinOp ==\n  lhs: BinOp <\n    lhs: IntLit 1\n    rhs: IntLit 2\n  rhs: Name true"
    );
}

#[test]
fn subB_and_antes_de_or() {
    assert_eq!(
        dump_expr("a or b and c"),
        "Or\n  Name a\n  And\n    Name b\n    Name c"
    );
}

#[test]
fn subB_not_e_mais_forte_que_and() {
    assert_eq!(dump_expr("not a and b"), "And\n  Not\n    Name a\n  Name b");
}

#[test]
fn subB_or_e_and_encadeados_viram_um_unico_no_com_n_operandos() {
    assert_eq!(dump_expr("a or b or c"), "Or\n  Name a\n  Name b\n  Name c");
    assert_eq!(
        dump_expr("a and b and c"),
        "And\n  Name a\n  Name b\n  Name c"
    );
}

#[test]
fn subB_not_e_prefixo_recursivo() {
    assert_eq!(dump_expr("not not a"), "Not\n  Not\n    Name a");
}

#[test]
fn subB_atribuicao_e_associativa_a_direita() {
    assert_eq!(
        dump_expr("a = b = 1"),
        "Assign target=a\n  Assign target=b\n    IntLit 1"
    );
}

#[test]
fn subB_parenteses_nao_geram_no_proprio() {
    assert_eq!(
        dump_expr("(1 + 2) * 3"),
        "BinOp *\n  lhs: BinOp +\n    lhs: IntLit 1\n    rhs: IntLit 2\n  rhs: IntLit 3"
    );
    assert_eq!(dump_expr("((1))"), "IntLit 1");
}

#[test]
fn subB_comparacoes_encadeadas_sao_esquerdistas() {
    assert_eq!(
        dump_expr("1 < 2 < 3"),
        "BinOp <\n  lhs: BinOp <\n    lhs: IntLit 1\n    rhs: IntLit 2\n  rhs: IntLit 3"
    );
}

// -------------------------------------------------------------------------------------------
// Subetapa C — primary: literais, nomes, chamadas, iif/for/try, listas, blocos
// -------------------------------------------------------------------------------------------

#[test]
fn subC_literais_preservam_texto_bruto_e_decodificam_escaping_de_string() {
    assert_eq!(dump_const_expr("42"), "IntLit 42");
    assert_eq!(dump_const_expr("4.5e1"), "RealLit 4.5e1");
    // String decodificada e re-escapada por `{:?}` no dump.
    assert_eq!(dump_const_expr(r#""a\nb""#), r#"StrLit "a\nb""#);
    assert_eq!(dump_const_expr(r#"'\t'"#), r#"StrLit "\t""#);
    assert_eq!(dump_const_expr(r#""c:\\d""#), r#"StrLit "c:\\d""#);
}

#[test]
fn subC_qualified_name_com_e_sem_alias() {
    assert_eq!(dump_const_expr("x"), "Name x");
    assert_eq!(dump_const_expr("m.sqrt"), "Name m.sqrt");
}

#[test]
fn subC_chamada_sem_argumento_e_com_argumento() {
    assert_eq!(dump_expr("f()"), "Call\n  target: Name f");
    assert_eq!(dump_expr("f(1)"), "Call\n  target: Name f\n  arg: IntLit 1");
    assert_eq!(
        dump_expr("m.f(x + 1)"),
        "Call\n  target: Name m.f\n  arg: BinOp +\n    lhs: Name x\n    rhs: IntLit 1"
    );
}

#[test]
fn subC_indexacao_do_resultado_de_uma_chamada() {
    // `indexed = primary { "[" ... }` e `primary` inclui `fn_call`, então `f(1)[0]` é válido.
    assert_eq!(
        dump_expr("f(1)[0]"),
        "Indexed\n  base: Call\n    target: Name f\n    arg: IntLit 1\n  idx[0]: IntLit 0"
    );
}

#[test]
fn subC_chamar_o_resultado_de_uma_chamada_nao_e_gramatical() {
    // `fn_call = qualified_name "(" [ expression ] ")"`: o alvo tem de ser um nome qualificado,
    // nunca uma chamada. `f(1)(2)` é erro de sintaxe (não existe aplicação de função de ordem
    // superior direta na gramática).
    // O `(` depois de `f(1)` aparece onde se esperava `;` — é esse o erro reportado.
    let (kind, bad_span) = parse_err("main = fn(x){ f(1)(2); };");
    assert_eq!(kind, SyntaxErrorKind::MissingSemicolon);
    // `main`(1-4) `=`(6) `fn`(8-9) `(`(10) `x`(11) `)`(12) `{`(13) `f`(15) `(`(16) `1`(17) `)`(18)
    // e o `(` ofensor na coluna 19.
    assert_eq!(bad_span, span(1, 19, 1, 20));
}

#[test]
fn subC_iif_tem_tres_filhos_rotulados() {
    assert_eq!(
        dump_expr("iif(1, 2, 3)"),
        "Iif\n  cond: IntLit 1\n  then: IntLit 2\n  else: IntLit 3"
    );
}

#[test]
fn subC_for_tem_init_cond_step_body() {
    assert_eq!(
        dump_expr("for(i = 0; i < 3; i = i + 1) { i; }"),
        "For\n  init: Assign target=i\n    IntLit 0\n  cond: BinOp <\n    lhs: Name i\n    rhs: IntLit 3\n  step: Assign target=i\n    BinOp +\n      lhs: Name i\n      rhs: IntLit 1\n  body: Block\n    Name i"
    );
}

#[test]
fn subC_try_envolve_uma_expressao() {
    assert_eq!(
        dump_expr("try(f(1))"),
        "Try\n  Call\n    target: Name f\n    arg: IntLit 1"
    );
}

#[test]
fn subC_listas_vazias_e_com_elementos() {
    assert_eq!(dump_const_expr("[]"), "Bracket");
    assert_eq!(
        dump_const_expr("[1, 2, 3]"),
        "Bracket\n  IntLit 1\n  IntLit 2\n  IntLit 3"
    );
    assert_eq!(
        dump_const_expr("[[1], [2]]"),
        "Bracket\n  Bracket\n    IntLit 1\n  Bracket\n    IntLit 2"
    );
}

#[test]
fn subC_bloco_vazio_e_bloco_com_statements() {
    assert_eq!(dump_expr("{}"), "Block");
    assert_eq!(dump_expr("{ 1; }"), "Block\n  IntLit 1");
    assert_eq!(dump_expr("{ 1; 2; }"), "Block\n  IntLit 1\n  IntLit 2");
}

#[test]
fn subC_bloco_dentro_de_bloco() {
    assert_eq!(
        dump_expr("{ { 1; }; 2; }"),
        "Block\n  Block\n    IntLit 1\n  IntLit 2"
    );
}

#[test]
fn subC_fndef_no_nivel_de_modulo_vira_initializer_fndef() {
    let module = parse_ok("f = fn(x){ x; };");
    match &module.definitions[0].initializer {
        Initializer::FnDef(f) => match &f.param {
            ParamPattern::Name { name, ty, .. } => {
                assert_eq!(name, "x");
                assert!(ty.is_none());
            }
            other => panic!("esperava parâmetro simples, veio {:?}", other),
        },
        other => panic!("esperava FnDef, veio {:?}", other),
    }
}

#[test]
fn subC_fndef_dentro_de_bloco_vira_expressao_fndef() {
    assert_eq!(
        dump_expr("f = fn(y){ y; }"),
        "Assign target=f\n  FnDef param=y\n    Block\n      Name y"
    );
}

// -------------------------------------------------------------------------------------------
// Subetapa D — indexed: colchetes encadeados e o açúcar `v[a, b]`
// -------------------------------------------------------------------------------------------

#[test]
fn subD_indexacao_simples() {
    assert_eq!(
        dump_const_expr("v[0]"),
        "Indexed\n  base: Name v\n  idx[0]: IntLit 0"
    );
}

#[test]
fn subD_indexacao_encadeada_gera_indexed_aninhados() {
    assert_eq!(
        dump_const_expr("v[0][1]"),
        "Indexed\n  base: Indexed\n    base: Name v\n    idx[0]: IntLit 0\n  idx[0]: IntLit 1"
    );
}

#[test]
fn subD_acucar_de_indices_multiplos_em_um_unico_no() {
    assert_eq!(
        dump_const_expr("v[0, 1, 2]"),
        "Indexed\n  base: Name v\n  idx[0]: IntLit 0\n  idx[1]: IntLit 1\n  idx[2]: IntLit 2"
    );
    assert_eq!(
        dump_const_expr("v[0, 1][2]"),
        "Indexed\n  base: Indexed\n    base: Name v\n    idx[0]: IntLit 0\n    idx[1]: IntLit 1\n  idx[0]: IntLit 2"
    );
}

#[test]
fn subD_indices_sao_expressoes_completas() {
    assert_eq!(
        dump_const_expr("v[i + 1, j * 2]"),
        "Indexed\n  base: Name v\n  idx[0]: BinOp +\n    lhs: Name i\n    rhs: IntLit 1\n  idx[1]: BinOp *\n    lhs: Name j\n    rhs: IntLit 2"
    );
}

#[test]
fn subD_indexacao_com_dois_indices_e_um_no_so_e_nao_dois_encadeados() {
    let module = parse_ok("r = v[0, 1];");
    match &module.definitions[0].initializer {
        Initializer::ConstExpr(Expr::Indexed { base, indices, .. }) => {
            assert_eq!(indices.len(), 2);
            assert!(matches!(**base, Expr::QualifiedName { .. }));
        }
        other => panic!("esperava Indexed, veio {:?}", other),
    }
}

#[test]
fn subD_indexacao_sem_fechamento_e_erro() {
    let (kind, _) = parse_err("r = v[0;");
    assert_eq!(kind, SyntaxErrorKind::UnexpectedToken);
}

// -------------------------------------------------------------------------------------------
// Subetapa E — padrões e anotações de tipo
// -------------------------------------------------------------------------------------------

#[test]
fn subE_parametro_simples_com_e_sem_anotacao() {
    assert_eq!(
        param_of("f = fn(x){ x; };"),
        ParamPattern::Name {
            name: "x".to_string(),
            ty: None,
            span: span(1, 8, 1, 9)
        }
    );
    assert_eq!(
        param_of("f = fn(x:int){ x; };"),
        ParamPattern::Name {
            name: "x".to_string(),
            ty: Some(TypeAnnotation::Int),
            span: span(1, 8, 1, 13)
        }
    );
}

#[test]
fn subE_padrao_de_lista_aninhado_no_parametro() {
    assert_eq!(
        param_of("f = fn([[a, b], c]){ a; };"),
        ParamPattern::List(
            vec![
                ParamPattern::List(
                    vec![
                        ParamPattern::Name {
                            name: "a".to_string(),
                            ty: None,
                            span: span(1, 10, 1, 11)
                        },
                        ParamPattern::Name {
                            name: "b".to_string(),
                            ty: None,
                            span: span(1, 13, 1, 14)
                        },
                    ],
                    span(1, 9, 1, 15)
                ),
                ParamPattern::Name {
                    name: "c".to_string(),
                    ty: None,
                    span: span(1, 17, 1, 18)
                },
            ],
            span(1, 8, 1, 19)
        )
    );
}

#[test]
fn subE_todas_as_formas_de_tipo() {
    let cases = [
        ("bool", TypeAnnotation::Bool),
        ("int", TypeAnnotation::Int),
        ("real", TypeAnnotation::Real),
        ("string", TypeAnnotation::String),
        ("fn", TypeAnnotation::Fn),
        ("[]", TypeAnnotation::AnyList),
        ("[int]", TypeAnnotation::ListOf(Box::new(TypeAnnotation::Int))),
        (
            "[int, string]",
            TypeAnnotation::Tuple(vec![TypeAnnotation::Int, TypeAnnotation::String]),
        ),
        (
            "[[int], real]",
            TypeAnnotation::Tuple(vec![
                TypeAnnotation::ListOf(Box::new(TypeAnnotation::Int)),
                TypeAnnotation::Real,
            ]),
        ),
    ];
    for (text, expected) in cases {
        match param_of(&format!("f = fn(x:{}){{ x; }};", text)) {
            ParamPattern::Name { ty, .. } => assert_eq!(ty, Some(expected), "tipo {:?}", text),
            other => panic!("esperava padrão simples, veio {:?}", other),
        }
    }
}

#[test]
fn subE_nome_de_tipo_desconhecido_e_erro() {
    let (kind, _) = parse_err("f = fn(x:qualquer){ x; };");
    assert_eq!(kind, SyntaxErrorKind::UnexpectedToken);
}

#[test]
fn subE_palavras_chave_nao_sao_nomes_de_tipo() {
    // `not` é keyword e não um tipo válido.
    let (kind, _) = parse_err("f = fn(x:not){ x; };");
    assert_eq!(kind, SyntaxErrorKind::UnexpectedToken);
}

#[test]
fn subE_padrao_de_lista_em_atribuicao() {
    assert_eq!(
        dump_expr("[a, [b, c]] = [1, [2, 3]]"),
        "Assign target=[a, [b, c]]\n  Bracket\n    IntLit 1\n    Bracket\n      IntLit 2\n      IntLit 3"
    );
}

#[test]
fn subE_padrao_de_lista_vazio_eh_aceito() {
    assert_eq!(dump_expr("[] = []"), "Assign target=[]\n  Bracket");
}

// -------------------------------------------------------------------------------------------
// Subetapa F — atribuição e fn_def
// -------------------------------------------------------------------------------------------

#[test]
fn subF_atribuicao_simples() {
    assert_eq!(dump_expr("a = 1"), "Assign target=a\n  IntLit 1");
}

#[test]
fn subF_alvo_indexado_e_invalido() {
    // `  x[0] = 1;` — o `=` está na coluna 8.
    let (kind, bad_span) = parse_err("main = fn(x){\n  x[0] = 1;\n};");
    assert_eq!(kind, SyntaxErrorKind::InvalidAssignmentTarget);
    assert_eq!(bad_span, span(2, 8, 2, 9));
}

#[test]
fn subF_alvo_chamada_de_funcao_e_invalido() {
    let (kind, _) = parse_err("main = fn(x){ f(1) = 2; };");
    assert_eq!(kind, SyntaxErrorKind::InvalidAssignmentTarget);
}

#[test]
fn subF_alvo_literal_e_invalido() {
    // `param_pattern` é obrigatório na gramática, então os blocos usam `fn(x)`.
    for source in ["main = fn(x){ 1 = 2; };", "main = fn(x){ \"s\" = 2; };"] {
        let (kind, _) = parse_err(source);
        assert_eq!(
            kind,
            SyntaxErrorKind::InvalidAssignmentTarget,
            "fonte {:?}",
            source
        );
    }
}

#[test]
fn subF_alvo_qualificado_e_invalido() {
    // `m.x = 1` não é alvo válido: binding_pattern é só identificador ou lista.
    let (kind, _) = parse_err("main = fn(x){ m.x = 1; };");
    assert_eq!(kind, SyntaxErrorKind::InvalidAssignmentTarget);
}

#[test]
fn subF_atribuicao_encadeada_a_direita_com_tres_niveis() {
    assert_eq!(
        dump_expr("a = b = c = 0"),
        "Assign target=a\n  Assign target=b\n    Assign target=c\n      IntLit 0"
    );
}

// -------------------------------------------------------------------------------------------
// Subetapa G — blocos e `;` obrigatório
// -------------------------------------------------------------------------------------------

#[test]
fn subG_falta_de_ponto_e_virgula_em_statement_de_bloco() {
    let (kind, _) = parse_err("f = fn(x){ x };");
    assert_eq!(kind, SyntaxErrorKind::MissingSemicolon);
}

#[test]
fn subG_falta_de_ponto_e_virgula_entre_definicoes() {
    let (kind, _) = parse_err("a = 1\nb = 2;");
    assert_eq!(kind, SyntaxErrorKind::MissingSemicolon);
}

#[test]
fn subG_bloco_nao_fechado_e_erro() {
    let (kind, _) = parse_err("f = fn(x){ x;");
    assert_eq!(kind, SyntaxErrorKind::UnexpectedToken);
}

#[test]
fn subG_ponto_e_virgula_extra_e_erro() {
    // `;;` — o segundo `;` aparece onde se esperava um inicializador.
    let (kind, _) = parse_err("a = 1;;");
    assert_eq!(kind, SyntaxErrorKind::UnexpectedToken);
}

// -------------------------------------------------------------------------------------------
// Subetapa H — adjacência de sinal unário (exemplos literais da Seção 4)
// -------------------------------------------------------------------------------------------

#[test]
fn subH_sinal_unario_colado_ao_numero_e_valido() {
    assert_eq!(dump_expr("-3"), "Unary - adj=true\n  IntLit 3");
    assert_eq!(dump_expr("+3"), "Unary + adj=true\n  IntLit 3");
    assert_eq!(
        dump_expr("2 * -3"),
        "BinOp *\n  lhs: IntLit 2\n  rhs: Unary - adj=true\n    IntLit 3"
    );
    assert_eq!(
        dump_expr("2 *-3"),
        "BinOp *\n  lhs: IntLit 2\n  rhs: Unary - adj=true\n    IntLit 3"
    );
    assert_eq!(dump_expr("-3.5e2"), "Unary - adj=true\n  RealLit 3.5e2");
}

#[test]
fn subH_sinal_unario_separado_do_numero_e_erro() {
    // Os três exemplos que a Seção 4 marca explicitamente como inválidos, cada um com o span do
    // seu próprio sinal: em `r = - 3;` o sinal está na coluna 5; em `r = 2 *- 3;` na coluna 8.
    let cases = [("- 3", 5), ("+ 3", 5), ("2 *- 3", 8)];
    for (source, column) in cases {
        let (kind, bad_span) = parse_err(&format!("r = {};", source));
        assert_eq!(kind, SyntaxErrorKind::SignNotAdjacent, "fonte {:?}", source);
        assert_eq!(
            bad_span,
            span(1, column, 1, column + 1),
            "fonte {:?}",
            source
        );
    }
}

#[test]
fn subH_subtracao_binaria_permite_espacos() {
    assert_eq!(
        dump_expr("2 - 3"),
        "BinOp -\n  lhs: IntLit 2\n  rhs: IntLit 3"
    );
}

#[test]
fn subH_sinal_unario_sobre_nao_literal_nao_exige_adjacencia() {
    // A restrição da spec vale só para literal numérico; `-x` e `-(1 + 2)` são livres.
    assert_eq!(dump_expr("- x"), "Unary - adj=false\n  Name x");
    assert_eq!(
        dump_expr("- (1 + 2)"),
        "Unary - adj=false\n  BinOp +\n    lhs: IntLit 1\n    rhs: IntLit 2"
    );
    assert_eq!(dump_expr("-x"), "Unary - adj=true\n  Name x");
}

#[test]
fn subH_comentario_entre_sinal_e_numero_quebra_a_adjacencia() {
    let (kind, _) = parse_err("r = -/* c */3;");
    assert_eq!(kind, SyntaxErrorKind::SignNotAdjacent);
}

#[test]
fn subH_sinal_unario_separado_de_parentese_nao_e_erro() {
    // O sinal precede `(`, não um literal: nenhuma restrição de adjacência.
    assert_eq!(
        dump_expr("+ (1)"),
        "Unary + adj=false\n  IntLit 1"
    );
}

// -------------------------------------------------------------------------------------------
// Subetapa I — nível de módulo: imports, definições e const_expr
// -------------------------------------------------------------------------------------------

#[test]
fn subI_import_com_e_sem_alias() {
    let module = parse_ok("import(\"math\");\nimport(\"strings\", \"s\");\nr = 1;");
    assert_eq!(module.imports.len(), 2);
    assert_eq!(module.imports[0].name, "math");
    assert_eq!(module.imports[0].alias, None);
    assert_eq!(module.imports[0].effective_alias(), "math");
    assert_eq!(module.imports[1].name, "strings");
    assert_eq!(module.imports[1].alias, Some("s".to_string()));
    assert_eq!(module.imports[1].effective_alias(), "s");
}

#[test]
fn subI_import_depois_de_definicao_e_erro() {
    let (kind, bad_span) = parse_err("f = 1;\nimport(\"math\");");
    assert_eq!(kind, SyntaxErrorKind::ImportNotAtModuleStart);
    // `import` ocupa as colunas 1-6.
    assert_eq!(bad_span, span(2, 1, 2, 7));
}

#[test]
fn subI_import_dentro_de_bloco_e_erro() {
    let (kind, _) = parse_err("f = fn(x){ import(\"math\"); };");
    assert_eq!(kind, SyntaxErrorKind::ImportNotAtModuleStart);
}

#[test]
fn subI_const_expr_aceita_expressoes_puras() {
    for source in [
        "r = 1 + 2 * 3;",
        "r = [1, 2, 3];",
        "r = -1;",
        "r = not true;",
        "r = m.x;",
        "r = [1, 2][0];",
        "r = v[0, 1];",
    ] {
        parse_ok(source);
    }
}

#[test]
fn subI_chamada_de_funcao_em_const_expr_e_erro() {
    let (kind, bad_span) = parse_err("f = len(\"abc\");");
    assert_eq!(kind, SyntaxErrorKind::InvalidConstExpr);
    // O span cobre a chamada inteira: `len("abc")` ocupa as colunas 5 a 14.
    assert_eq!(bad_span, span(1, 5, 1, 15));
}

#[test]
fn subI_chamada_de_funcao_aninhada_em_const_expr_e_erro() {
    let (kind, _) = parse_err("f = 1 + len(\"abc\");");
    assert_eq!(kind, SyntaxErrorKind::InvalidConstExpr);
}

#[test]
fn subI_bloco_iif_for_atribuicao_em_const_expr_sao_erro() {
    for source in [
        "f = { 1; };",
        "f = iif(true, 1, 2);",
        "f = for(i = 0; i < 1; i = i + 1) { i; };",
        "f = a = 1;",
    ] {
        let (kind, _) = parse_err(source);
        assert_eq!(kind, SyntaxErrorKind::InvalidConstExpr, "fonte {:?}", source);
    }
}

#[test]
fn subI_chamada_dentro_de_fndef_e_permitida() {
    // A restrição de const_expr vale só para inicializadores de nível de módulo que não são
    // fn_def; dentro de um corpo de função tudo é permitido.
    parse_ok("main = fn(x){ len(x); };");
}

#[test]
fn subI_chamada_aninhada_em_lista_de_const_expr_e_erro() {
    let (kind, _) = parse_err("f = [[len(\"a\")], 2];");
    assert_eq!(kind, SyntaxErrorKind::InvalidConstExpr);
}

// -------------------------------------------------------------------------------------------
// Subetapa J — programa vazio e multi-argumento
// -------------------------------------------------------------------------------------------

#[test]
fn subJ_programa_vazio_e_erro() {
    let (kind, bad_span) = parse_err("// nada aqui\n");
    assert_eq!(kind, SyntaxErrorKind::EmptyProgram);
    // O `Eof` está na linha 2, coluna 1.
    assert_eq!(bad_span, span(2, 1, 2, 1));
}

#[test]
fn subJ_programa_com_apenas_imports_e_vazio() {
    let (kind, _) = parse_err("import(\"math\");");
    assert_eq!(kind, SyntaxErrorKind::EmptyProgram);
}

#[test]
fn subJ_programa_so_com_espacos_e_vazio() {
    let (kind, _) = parse_err("\n\n   \n");
    assert_eq!(kind, SyntaxErrorKind::EmptyProgram);
}

#[test]
fn subJ_chamada_com_dois_argumentos_e_erro() {
    // `main = fn(x){ f(1, 2); };` — a vírgula está na coluna 18.
    let (kind, bad_span) = parse_err("main = fn(x){ f(1, 2); };");
    assert_eq!(kind, SyntaxErrorKind::MultiArgCall);
    assert_eq!(bad_span, span(1, 18, 1, 19));
}

#[test]
fn subG_param_pattern_e_obrigatorio_na_gramatica() {
    // `fn_def = "fn" "(" param_pattern ")" block` — não existe forma sem parâmetro.
    let (kind, _) = parse_err("main = fn(){ 1; };");
    assert_eq!(kind, SyntaxErrorKind::UnexpectedToken);
}

#[test]
fn subJ_chamada_com_varios_argumentos_tambem_e_erro() {
    let (kind, _) = parse_err("main = fn(x){ f(1, 2, 3); };");
    assert_eq!(kind, SyntaxErrorKind::MultiArgCall);
}

#[test]
fn subJ_chamada_com_um_unico_argumento_que_e_lista_e_valida() {
    // `f([1, 2])` é uma chamada de um argumento só (uma lista), não multi-argumento.
    assert_eq!(
        dump_expr("f([1, 2])"),
        "Call\n  target: Name f\n  arg: Bracket\n    IntLit 1\n    IntLit 2"
    );
}

#[test]
fn subJ_chamada_com_expressao_de_virgula_em_iif_e_aceita() {
    // `iif` usa vírgulas legitimamente; só a chamada de função é restrita a um argumento.
    assert_eq!(
        dump_expr("iif(a, b, c)"),
        "Iif\n  cond: Name a\n  then: Name b\n  else: Name c"
    );
}

// -------------------------------------------------------------------------------------------
// Exemplos literais da Seção 4
// -------------------------------------------------------------------------------------------

#[test]
fn sec4_fatorial_recursivo_e_parseado() {
    let module = parse_ok("fat = fn(x){ iif(x<=1, 1, x*fat(x-1)); };");
    assert_eq!(module.definitions.len(), 1);
    assert_eq!(module.definitions[0].name, "fat");
    match &module.definitions[0].initializer {
        Initializer::FnDef(f) => {
            assert_eq!(f.body.statements.len(), 1);
            assert!(matches!(f.body.statements[0], Stmt::Expr(Expr::Iif { .. })));
        }
        other => panic!("esperava FnDef, veio {:?}", other),
    }
}

#[test]
fn sec4_atribuicao_de_fndef_a_outro_nome_e_parseada_em_bloco() {
    // A Seção 4 traz `a = b = fn(x){ ... };` como statement de um bloco.
    let dump = dump_expr("{ a = b = fn(x){ a(x - 1); }; a(1); }");
    assert!(
        dump.starts_with("Block\n  Assign target=a\n    Assign target=b\n      FnDef param=x"),
        "dump inesperado:\n{}",
        dump
    );
}

#[test]
fn sec4_atribuicao_de_fndef_em_nivel_de_modulo_e_invalid_const_expr() {
    // Em nível de módulo o inicializador é `const_expr`, que rejeita atribuição.
    let (kind, _) = parse_err("a = b = fn(x){ iif(x == 0, 0, a(x - 1)); };");
    assert_eq!(kind, SyntaxErrorKind::InvalidConstExpr);
}

// -------------------------------------------------------------------------------------------
// Robustez da API (além da gramática)
// -------------------------------------------------------------------------------------------

#[test]
fn api_parser_com_slice_vazia_reporta_programa_vazio() {
    // `tokenize` nunca produz uma slice vazia, mas `Parser::new` é público: em vez de estourar o
    // índice, o parser trata como programa vazio.
    let (kind, bad_span) = match Parser::new(&[]).parse_module() {
        Ok(module) => panic!("esperava erro, veio {:?} definições", module.definitions.len()),
        Err(e) => (e.kind, e.span),
    };
    assert_eq!(kind, SyntaxErrorKind::EmptyProgram);
    assert_eq!(bad_span, span(1, 1, 1, 1));
}

#[test]
fn api_span_da_expressao_cobre_operandos() {
    let module = parse_ok("r = 1 + 2;");
    let expr = match &module.definitions[0].initializer {
        Initializer::ConstExpr(expr) => expr,
        other => panic!("esperava const_expr, veio {:?}", other),
    };
    // `1 + 2` vai da coluna 5 à 10 (exclusivo).
    assert_eq!(expr.span(), span(1, 5, 1, 10));
}

#[test]
fn api_import_decl_guarda_span_do_import() {
    let module = parse_ok("import(\"math\");\nr = 1;");
    // `import("math");` ocupa as colunas 1-15.
    assert_eq!(module.imports[0].span, span(1, 1, 1, 15));
}

//! Testes unitários do Lexer, organizados pelos grupos históricos 1–7.
//!
//! Ordem das subetapas (cada seção testa só o que já existe até ela):
//! 1. Núcleo (avanço por codepoint, linha/coluna, EOF, espaços em branco, comentários)
//! 2. Números (`int_lit`, `real_lit`, `malformed-number`)
//! 3. Strings (delimitação, conteúdo bruto, multilinha, não terminada)
//! 4. Identificadores e palavras-chave (ASCII-only)
//! 5. Operadores e pontuação (incl. `.` ambíguo)
//! 6. Metadado de adjacência de sinal (`adjacent_to_next`)
//! 7. Erros e posições (span de cada erro, parada no primeiro erro)
//!
//! A suíte golden-file equivalente (fixtures de `tests/lexer/cases/`) vive em
//! `tests/lexer_fixtures.rs`; estas duas suítes juntas fecham a etapa.

use xprc::lexer::{LexErrorKind, Lexer, Span, Token, TokenKind};

// -------------------------------------------------------------------------------------------
// Utilidades
// -------------------------------------------------------------------------------------------

fn lex(source: &str) -> Vec<Token> {
    Lexer::new(source).tokenize().unwrap_or_else(|e| {
        panic!(
            "tokenização de {:?} falhou: {:?} em {:?}",
            source, e.kind, e.span
        )
    })
}

/// Tokeniza esperando exatamente um problema léxico.
fn lex_err(source: &str) -> (LexErrorKind, Span) {
    match Lexer::new(source).tokenize() {
        Ok(tokens) => panic!(
            "esperava erro léxico em {:?}, mas veio sucesso com {} tokens: {:?}",
            source,
            tokens.len(),
            tokens.iter().map(|t| &t.kind).collect::<Vec<_>>()
        ),
        Err(e) => (e.kind, e.span),
    }
}

fn span(line_start: u32, col_start: u32, line_end: u32, col_end: u32) -> Span {
    Span {
        line_start,
        col_start,
        line_end,
        col_end,
    }
}

/// Só os `TokenKind`, sem o `Eof` final (o `Eof` é conferido à parte).
fn kinds(source: &str) -> Vec<TokenKind> {
    let tokens = lex(source);
    assert_eq!(
        tokens.last().map(|t| t.kind.clone()),
        Some(TokenKind::Eof),
        "a sequência de {:?} não terminou em Eof",
        source
    );
    tokens[..tokens.len() - 1]
        .iter()
        .map(|t| t.kind.clone())
        .collect()
}

fn int(text: &str) -> TokenKind {
    TokenKind::IntLit(text.to_string())
}

fn real(text: &str) -> TokenKind {
    TokenKind::RealLit(text.to_string())
}

fn ident(name: &str) -> TokenKind {
    TokenKind::Ident(name.to_string())
}

fn string_dq(raw: &str) -> TokenKind {
    TokenKind::StringLit {
        raw: raw.to_string(),
        quote: '"',
    }
}

fn string_sq(raw: &str) -> TokenKind {
    TokenKind::StringLit {
        raw: raw.to_string(),
        quote: '\'',
    }
}

/// Posição e kind de cada token, na ordem, **incluindo** o `Eof`.
fn positions_and_kinds(source: &str) -> Vec<(&'static str, Span)> {
    let tokens = lex(source);
    tokens.iter().map(|t| (kind_tag(&t.kind), t.span)).collect()
}

fn kind_tag(kind: &TokenKind) -> &'static str {
    match kind {
        TokenKind::IntLit(_) => "IntLit",
        TokenKind::RealLit(_) => "RealLit",
        TokenKind::StringLit { .. } => "StringLit",
        TokenKind::Ident(_) => "Ident",
        TokenKind::KwFn => "KwFn",
        TokenKind::KwIif => "KwIif",
        TokenKind::KwFor => "KwFor",
        TokenKind::KwTry => "KwTry",
        TokenKind::KwImport => "KwImport",
        TokenKind::KwAnd => "KwAnd",
        TokenKind::KwOr => "KwOr",
        TokenKind::KwNot => "KwNot",
        TokenKind::Plus => "Plus",
        TokenKind::Minus => "Minus",
        TokenKind::Star => "Star",
        TokenKind::Slash => "Slash",
        TokenKind::Percent => "Percent",
        TokenKind::Lt => "Lt",
        TokenKind::Gt => "Gt",
        TokenKind::Le => "Le",
        TokenKind::Ge => "Ge",
        TokenKind::EqEq => "EqEq",
        TokenKind::NotEq => "NotEq",
        TokenKind::Eq => "Eq",
        TokenKind::LParen => "LParen",
        TokenKind::RParen => "RParen",
        TokenKind::LBrace => "LBrace",
        TokenKind::RBrace => "RBrace",
        TokenKind::LBracket => "LBracket",
        TokenKind::RBracket => "RBracket",
        TokenKind::Comma => "Comma",
        TokenKind::Semi => "Semi",
        TokenKind::Colon => "Colon",
        TokenKind::Dot => "Dot",
        TokenKind::Eof => "Eof",
    }
}

// -------------------------------------------------------------------------------------------
// Subetapa 1 — Núcleo: avanço por codepoint, linha/coluna, EOF, espaços, comentários
// -------------------------------------------------------------------------------------------

#[test]
fn sub1_fonte_vazia_so_tem_eof_em_1_1() {
    assert_eq!(positions_and_kinds(""), vec![("Eof", span(1, 1, 1, 1))]);
}

#[test]
fn sub1_eof_apos_espacos_e_quebras_de_linha() {
    assert_eq!(
        positions_and_kinds("  \n\t\n "),
        vec![("Eof", span(3, 2, 3, 2))]
    );
}

#[test]
fn sub1_espacos_em_branco_sao_ignorados() {
    assert_eq!(
        positions_and_kinds("  a\t\tb\r\nc "),
        vec![
            ("Ident", span(1, 3, 1, 4)),
            ("Ident", span(1, 6, 1, 7)),
            ("Ident", span(2, 1, 2, 2)),
            ("Eof", span(2, 3, 2, 3)),
        ]
    );
}

#[test]
fn sub1_linhas_e_colunas_contam_codepoints_nao_bytes() {
    // Acentos dentro de string não deslocam a coluna seguinte, porque a coluna é contada em
    // codepoints. `"ação"` ocupa 6 codepoints (aspas inclusas) e 8 bytes.
    assert_eq!(
        positions_and_kinds("\"ação\" x"),
        vec![
            ("StringLit", span(1, 1, 1, 7)),
            ("Ident", span(1, 8, 1, 9)),
            ("Eof", span(1, 9, 1, 9)),
        ]
    );
}

#[test]
fn sub1_comentario_de_linha_eh_ignorado() {
    assert_eq!(
        positions_and_kinds("// nada aqui\nx"),
        vec![("Ident", span(2, 1, 2, 2)), ("Eof", span(2, 2, 2, 2))]
    );
}

#[test]
fn sub1_comentario_de_linha_no_fim_do_arquivo_sem_newline() {
    assert_eq!(
        positions_and_kinds("x // fim"),
        vec![("Ident", span(1, 1, 1, 2)), ("Eof", span(1, 9, 1, 9))]
    );
}

#[test]
fn sub1_comentario_de_bloco_pode_cruzar_linhas() {
    // Linha 3 é `/* nao aninha */ b`: o `*/` fecha nas colunas 15-16 (comentários não
    // aninham, então o `/*` interno é texto), o espaço ocupa a 17 e `b` fica na coluna 18.
    assert_eq!(
        positions_and_kinds("a /* um\ndois\n/* nao aninha */ b"),
        vec![
            ("Ident", span(1, 1, 1, 2)),
            ("Ident", span(3, 18, 3, 19)),
            ("Eof", span(3, 19, 3, 19)),
        ]
    );
}

#[test]
fn sub1_comentario_de_bloco_vazio_e_contiguo() {
    // `a`(1) `/`(2) `*`(3) `*`(4) `/`(5) `b`(6) — o comentário `/**/` tem 4 codepoints e não
    // gera token; `b` começa na coluna 6.
    assert_eq!(
        positions_and_kinds("a/**/b"),
        vec![
            ("Ident", span(1, 1, 1, 2)),
            ("Ident", span(1, 6, 1, 7)),
            ("Eof", span(1, 7, 1, 7)),
        ]
    );
}

#[test]
fn sub1_barra_solitaria_eh_operador_e_nao_comentario() {
    // Duas barras isoladas (`/` seguida de espaço) são operadores `Slash`, não comentário.
    assert_eq!(kinds("/ /"), vec![TokenKind::Slash, TokenKind::Slash]);
    // `/*/` NÃO é um comentário fechado: o `*/` final não existe (sobram só `/`). A spec não
    // tem comentários aninhados nem fechamento parcial, então isso é `UnterminatedComment`.
    assert_eq!(
        lex_err("/*/"),
        (LexErrorKind::UnterminatedComment, span(1, 1, 1, 4))
    );
    // `/**/` fecha imediatamente e não gera token algum.
    assert_eq!(kinds("/**/"), Vec::<TokenKind>::new());
}

// -------------------------------------------------------------------------------------------
// Subetapa 2 — Números: int_lit, real_lit (4 formas), malformed-number
// -------------------------------------------------------------------------------------------

#[test]
fn sub2_inteiros_com_zeros_a_esquerda_preservam_o_texto_bruto() {
    assert_eq!(
        kinds("0 007 1234567890"),
        vec![int("0"), int("007"), int("1234567890")]
    );
}

#[test]
fn sub2_limites_do_i64_sao_aceitos() {
    assert_eq!(
        kinds("9223372036854775807"),
        vec![int("9223372036854775807")]
    );
}

#[test]
fn sub2_inteiro_fora_de_i64_eh_malformed_number() {
    assert_eq!(
        lex_err("9223372036854775808").0,
        LexErrorKind::MalformedNumber
    );
}

#[test]
fn sub2_reais_nas_quatro_formas_da_gramatica() {
    // int_num "." {digit} [exp] | int_num exp | "." int_num [exp]
    assert_eq!(
        kinds("1.0 3. 3e0 .5 1.5e-10 2E+3"),
        vec![
            real("1.0"),
            real("3."),
            real("3e0"),
            real(".5"),
            real("1.5e-10"),
            real("2E+3"),
        ]
    );
}

#[test]
fn sub2_real_com_expoente_sem_ponto_nao_e_validado_como_i64() {
    // Texto bruto é preservado exatamente como escrito, inclusive `+` e maiúsculas.
    assert_eq!(kinds("12E+34"), vec![real("12E+34")]);
}

#[test]
fn sub2_ponto_seguido_de_digito_inicia_real_mesmo_apos_identificador() {
    assert_eq!(kinds("x.5"), vec![ident("x"), real(".5")]);
}

#[test]
fn sub2_ponto_no_fim_de_um_real_nao_consome_o_proximo_numero() {
    assert_eq!(kinds("1.2.3"), vec![real("1.2"), real(".3")]);
}

#[test]
fn sub2_expoente_incompleto_nao_forma_real() {
    // `3e` não tem `int_num` após o `e`: o número é inteiro e `e` vira identificador.
    assert_eq!(kinds("3e"), vec![int("3"), ident("e")]);
    // `3e+` idem: `+` não é dígito, então o expoente não existe.
    assert_eq!(kinds("3e+"), vec![int("3"), ident("e"), TokenKind::Plus]);
}

// -------------------------------------------------------------------------------------------
// Subetapa 3 — Strings: delimitação sem decodificação, multilinha, não terminada
// -------------------------------------------------------------------------------------------

#[test]
fn sub3_conteudo_bruto_preservado_sem_decodificar_escapes() {
    assert_eq!(
        kinds(r#""linha\ncom\tescape" 'a\'b'"#),
        vec![string_dq(r"linha\ncom\tescape"), string_sq(r"a\'b")]
    );
}

#[test]
fn sub3_string_com_escape_de_barra_no_fim() {
    assert_eq!(kinds(r#""c:\\""#), vec![string_dq(r"c:\\")]);
}

#[test]
fn sub3_string_vazia() {
    assert_eq!(kinds("\"\" ''"), vec![string_dq(""), string_sq("")]);
}

#[test]
fn sub3_string_multilinha_usa_span_de_duas_linhas() {
    let tokens = lex("\"a\nbc\" x");
    assert_eq!(tokens[0].kind, string_dq("a\nbc"));
    assert_eq!(tokens[0].span, span(1, 1, 2, 4));
    assert_eq!(tokens[1].span, span(2, 5, 2, 6));
}

#[test]
fn sub3_string_pode_conter_aspas_do_outro_tipo_e_virgulas() {
    assert_eq!(
        kinds(r#"'diz "oi", 1; 2' "e 'isso'" "#),
        vec![string_sq("diz \"oi\", 1; 2"), string_dq("e 'isso'")]
    );
}

#[test]
fn sub3_string_nao_terminada_reporta_span_da_abertura_ate_o_fim() {
    assert_eq!(
        lex_err("x \"abc"),
        (LexErrorKind::UnterminatedString, span(1, 3, 1, 7))
    );
}

#[test]
fn sub3_string_nao_terminada_por_escape_no_fim() {
    assert_eq!(
        lex_err("\"abc\\"),
        (LexErrorKind::UnterminatedString, span(1, 1, 1, 6))
    );
}

// -------------------------------------------------------------------------------------------
// Subetapa 4 — Identificadores e palavras-chave
// -------------------------------------------------------------------------------------------

#[test]
fn sub4_as_oito_palavras_chave() {
    assert_eq!(
        kinds("fn iif for try import and or not"),
        vec![
            TokenKind::KwFn,
            TokenKind::KwIif,
            TokenKind::KwFor,
            TokenKind::KwTry,
            TokenKind::KwImport,
            TokenKind::KwAnd,
            TokenKind::KwOr,
            TokenKind::KwNot,
        ]
    );
}

#[test]
fn sub4_nomes_de_tipo_e_booleanos_nao_sao_palavras_chave() {
    assert_eq!(
        kinds("true false bool int real string number"),
        vec![
            ident("true"),
            ident("false"),
            ident("bool"),
            ident("int"),
            ident("real"),
            ident("string"),
            ident("number"),
        ]
    );
}

#[test]
fn sub4_prefixos_de_keyword_sao_identificadores() {
    assert_eq!(
        kinds("fnx iff forn try_ imported ands none _not"),
        vec![
            ident("fnx"),
            ident("iff"),
            ident("forn"),
            ident("try_"),
            ident("imported"),
            ident("ands"),
            ident("none"),
            ident("_not"),
        ]
    );
}

#[test]
fn sub4_identificadores_com_underscore_e_digitos() {
    assert_eq!(
        kinds("_ __ a1 _1 x_y9 A_b"),
        vec![
            ident("_"),
            ident("__"),
            ident("a1"),
            ident("_1"),
            ident("x_y9"),
            ident("A_b"),
        ]
    );
}

#[test]
fn sub4_caractere_nao_ascii_fora_de_string_eh_invalido_no_codepoint_ofensor() {
    assert_eq!(
        lex_err("café"),
        (LexErrorKind::InvalidCharacter, span(1, 4, 1, 5))
    );
    assert_eq!(
        lex_err("ação"),
        (LexErrorKind::InvalidCharacter, span(1, 2, 1, 3))
    );
}

#[test]
fn sub4_nao_ascii_inicia_erro_em_1_1() {
    assert_eq!(lex_err("π"), (LexErrorKind::InvalidCharacter, span(1, 1, 1, 2)));
}

// -------------------------------------------------------------------------------------------
// Subetapa 5 — Operadores e pontuação (incl. `.` ambíguo)
// -------------------------------------------------------------------------------------------

#[test]
fn sub5_todos_os_operadores_e_pontuacao() {
    assert_eq!(
        kinds("+ - * / % < > <= >= == != = ( ) { } [ ] , ; : ."),
        vec![
            TokenKind::Plus,
            TokenKind::Minus,
            TokenKind::Star,
            TokenKind::Slash,
            TokenKind::Percent,
            TokenKind::Lt,
            TokenKind::Gt,
            TokenKind::Le,
            TokenKind::Ge,
            TokenKind::EqEq,
            TokenKind::NotEq,
            TokenKind::Eq,
            TokenKind::LParen,
            TokenKind::RParen,
            TokenKind::LBrace,
            TokenKind::RBrace,
            TokenKind::LBracket,
            TokenKind::RBracket,
            TokenKind::Comma,
            TokenKind::Semi,
            TokenKind::Colon,
            TokenKind::Dot,
        ]
    );
}

#[test]
fn sub5_operadores_de_dois_caracteres_tem_span_de_dois() {
    let tokens = lex("== != <= >=");
    let spans: Vec<Span> = tokens[..4].iter().map(|t| t.span).collect();
    assert_eq!(
        spans,
        vec![
            span(1, 1, 1, 3),
            span(1, 4, 1, 6),
            span(1, 7, 1, 9),
            span(1, 10, 1, 12),
        ]
    );
}

#[test]
fn sub5_operadores_sem_espaco_nao_se_fundem() {
    // `=>` e `=<` não existem: cada codepoint é um token próprio.
    assert_eq!(
        kinds("= > <"),
        vec![TokenKind::Eq, TokenKind::Gt, TokenKind::Lt]
    );
    assert_eq!(kinds(">="), vec![TokenKind::Ge]);
    assert_eq!(kinds("=>"), vec![TokenKind::Eq, TokenKind::Gt]);
    assert_eq!(kinds("=<"), vec![TokenKind::Eq, TokenKind::Lt]);
}

#[test]
fn sub5_ponto_entre_identificadores_eh_dot() {
    assert_eq!(
        kinds("m.nome"),
        vec![ident("m"), TokenKind::Dot, ident("nome")]
    );
    assert_eq!(
        kinds("a.b.c"),
        vec![
            ident("a"),
            TokenKind::Dot,
            ident("b"),
            TokenKind::Dot,
            ident("c"),
        ]
    );
}

#[test]
fn sub5_ponto_seguido_de_digito_eh_real() {
    assert_eq!(kinds(".5"), vec![real(".5")]);
    assert_eq!(kinds("."), vec![TokenKind::Dot]);
    assert_eq!(kinds(".x"), vec![TokenKind::Dot, ident("x")]);
    assert_eq!(
        kinds("( .5 )"),
        vec![TokenKind::LParen, real(".5"), TokenKind::RParen]
    );
}

#[test]
fn sub5_caracteres_reservados_e_desconhecidos_sao_invalidos() {
    for source in ["&", "|", "^", "~", "@", "#", "$", "!", "?"] {
        assert_eq!(
            lex_err(source).0,
            LexErrorKind::InvalidCharacter,
            "esperava InvalidCharacter para {:?}",
            source
        );
    }
}

// -------------------------------------------------------------------------------------------
// Subetapa 6 — Metadado de adjacência de sinal (`adjacent_to_next`)
// -------------------------------------------------------------------------------------------

fn signs(source: &str) -> Vec<(TokenKind, bool)> {
    lex(source)
        .into_iter()
        .filter(|t| matches!(t.kind, TokenKind::Plus | TokenKind::Minus))
        .map(|t| (t.kind, t.adjacent_to_next))
        .collect()
}

#[test]
fn sub6_sinal_colado_ao_numero_eh_adjacente() {
    assert_eq!(signs("-3"), vec![(TokenKind::Minus, true)]);
    assert_eq!(signs("2*-3"), vec![(TokenKind::Minus, true)]);
    assert_eq!(signs("2 *-3"), vec![(TokenKind::Minus, true)]);
    assert_eq!(signs("+3"), vec![(TokenKind::Plus, true)]);
}

#[test]
fn sub6_espaco_entre_sinal_e_numero_nao_eh_adjacente() {
    assert_eq!(signs("- 3"), vec![(TokenKind::Minus, false)]);
    assert_eq!(signs("+ 3"), vec![(TokenKind::Plus, false)]);
}

#[test]
fn sub6_nova_linha_nao_eh_adjacencia() {
    assert_eq!(signs("-\n3"), vec![(TokenKind::Minus, false)]);
}

#[test]
fn sub6_comentario_nao_eh_adjacencia() {
    assert_eq!(signs("-/*c*/3"), vec![(TokenKind::Minus, false)]);
    assert_eq!(signs("-//c\n3"), vec![(TokenKind::Minus, false)]);
}

#[test]
fn sub6_adjacencia_vale_para_qualquer_token_seguinte() {
    // A decisão semântica (literal numérico ou não) fica no parser; o lexer só informa o fato.
    assert_eq!(signs("-x"), vec![(TokenKind::Minus, true)]);
    assert_eq!(signs("-(1)"), vec![(TokenKind::Minus, true)]);
    assert_eq!(signs("- x"), vec![(TokenKind::Minus, false)]);
    assert_eq!(signs("-[1]"), vec![(TokenKind::Minus, true)]);
}

#[test]
fn sub6_sinal_no_fim_do_arquivo_nao_eh_adjacente() {
    assert_eq!(signs("-"), vec![(TokenKind::Minus, false)]);
    assert_eq!(signs("- "), vec![(TokenKind::Minus, false)]);
}

#[test]
fn sub6_binario_colado_tambem_eh_marcado_adjacente() {
    // `a-1` é subtração, mas o lexer não sabe disso: a marca é apenas um fato léxico.
    let tokens = lex("a-1");
    assert_eq!(tokens[1].kind, TokenKind::Minus);
    assert!(tokens[1].adjacent_to_next);
}

#[test]
fn sub6_tokens_que_nao_sao_sinal_nunca_sao_marcados() {
    for token in lex("a 1 \"s\" ( ) == .5") {
        if !matches!(token.kind, TokenKind::Plus | TokenKind::Minus) {
            assert!(
                !token.adjacent_to_next,
                "token não-sinal {:?} marcado como adjacente",
                token.kind
            );
        }
    }
}

#[test]
fn sub6_dois_sinais_seguidos_sao_ambos_adjacentes() {
    assert_eq!(
        signs("--3"),
        vec![(TokenKind::Minus, true), (TokenKind::Minus, true)]
    );
}

// -------------------------------------------------------------------------------------------
// Subetapa 7 — Erros e posições
// -------------------------------------------------------------------------------------------

#[test]
fn sub7_span_de_malformed_number_cobre_o_inteiro() {
    assert_eq!(
        lex_err("  99999999999999999999"),
        (LexErrorKind::MalformedNumber, span(1, 3, 1, 23))
    );
}

#[test]
fn sub7_span_de_erro_em_linha_posterior() {
    assert_eq!(
        lex_err("x\ny\n  \"aberta"),
        (LexErrorKind::UnterminatedString, span(3, 3, 3, 10))
    );
}

#[test]
fn sub7_span_de_comentario_nao_fechado_cobre_do_inicio_ao_eof() {
    assert_eq!(
        lex_err("a /* nunca\nfecha"),
        (LexErrorKind::UnterminatedComment, span(1, 3, 2, 6))
    );
}

#[test]
fn sub7_compilacao_para_no_primeiro_erro_lexico() {
    // Vários problemas na mesma fonte: o primeiro encontrado é o reportado.
    assert_eq!(
        lex_err("x 99999999999999999999 @ \"aberta"),
        (LexErrorKind::MalformedNumber, span(1, 3, 1, 23))
    );
    assert_eq!(
        lex_err("x @ 99999999999999999999"),
        (LexErrorKind::InvalidCharacter, span(1, 3, 1, 4))
    );
}

#[test]
fn sub7_erro_descarta_tokens_parciais() {
    // `tokenize` devolve `Err` e nada mais: o chamador nunca vê os tokens válidos anteriores.
    let result = Lexer::new("a b c @").tokenize();
    assert!(result.is_err());
}

#[test]
fn sub7_spans_de_tokens_consecutivos_sao_contiguos() {
    let tokens = lex("a=1+2.5;");
    for window in tokens[..tokens.len() - 1].windows(2) {
        assert_eq!(
            window[0].span.line_end, window[1].span.line_start,
            "tokens em linhas diferentes inesperadamente"
        );
        assert_eq!(
            window[0].span.col_end, window[1].span.col_start,
            "tokens {:?} e {:?} não são contíguos",
            window[0].kind,
            window[1].kind
        );
    }
}

#[test]
fn sub7_token_multilinha_so_ocorre_em_string() {
    // Nenhum token fora de string pode ter `line_start != line_end`.
    for token in lex("a\nb\n1.5\n/*x\ny*/\nc \"d\ne\"") {
        if !matches!(token.kind, TokenKind::StringLit { .. } | TokenKind::Eof) {
            assert_eq!(
                token.span.line_start, token.span.line_end,
                "token {:?} cruzou linhas",
                token.kind
            );
        }
    }
}

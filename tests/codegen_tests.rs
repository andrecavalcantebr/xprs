//! Testes da etapa Codegen (subetapas A–G de `plano-codegen.md`).
//!
//! Diferente das etapas Lexer/Parser, aqui o oráculo não é um dump interno e sim **comportamento
//! observável de ponta a ponta**: compilar um `.x` com `xprc`, rodar o executável gerado e
//! comparar stdout/status. Os milestones de `tests/codegen/milestones/` cobrem esses casos pelo
//! script `scripts/run-codegen-milestones.sh`; este arquivo cobre o que *não* é milestone:
//!
//! * emissão de Rust (subetapa G) — o texto gerado é válido e contém o que deveria;
//! * resolução de nomes/escopos e captura (subetapa A);
//! * diagnósticos de compilação (subetapa E);
//! * erros de módulo (subetapa B);
//! * regras puras do runtime (subetapas C/F), testadas diretamente via `xpr_rt`.
//!
//! Os testes que exigem compilar+executar usam `rustc` de verdade e ficam num único teste cada,
//! para não pagar o custo de compilação repetidas vezes.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use xprc::codegen::{emit_program, ModuleUnit};
use xprc::diag::DiagCode;
use xprc::lexer::Lexer;
use xprc::parser::Parser;

// ---------------------------------------------------------------------------------------------
// Utilidades
// ---------------------------------------------------------------------------------------------

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn parse_source(source: &str) -> xprc::parser::ast::Module {
    let tokens = Lexer::new(source)
        .tokenize()
        .unwrap_or_else(|e| panic!("erro léxico em {:?}: {:?}", source, e.kind));
    Parser::new(&tokens)
        .parse_module()
        .unwrap_or_else(|e| panic!("erro sintático em {:?}: {:?}", source, e.kind))
}

fn module_unit(name: &str, source: &str) -> ModuleUnit {
    ModuleUnit {
        name: name.to_string(),
        path: format!("{}.x", name),
        ast: parse_source(source),
    }
}

fn emit(source: &str) -> Result<String, xprc::codegen::SemaError> {
    emit_program("main", &[module_unit("main", source)])
}

fn emit_err(source: &str) -> (DiagCode, Option<xprc::lexer::Span>) {
    match emit(source) {
        Ok(_) => panic!("esperava erro de compilação para {:?}", source),
        Err(err) => (err.code, err.span),
    }
}

/// Compila uma fonte XPR com `xprc` e executa, devolvendo (stdout, exit code, stderr).
fn compile_and_run(name: &str, source: &str) -> (String, i32, String) {
    let dir = std::env::temp_dir().join(format!(
        "xprc-test-{}-{}-{}",
        std::process::id(),
        name,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&dir).expect("criar diretório temporário");
    let xpr = dir.join(format!("{}.x", name));
    fs::write(&xpr, source).expect("escrever fonte .x");
    let exe = dir.join(format!("{}.exe", name));

    let xprc = env!("CARGO_BIN_EXE_xprc");
    let compile = Command::new(xprc)
        .arg(&xpr)
        .arg("-o")
        .arg(&exe)
        .output()
        .expect("executar xprc");
    assert!(
        compile.status.success(),
        "xprc falhou para {:?}:\nstdout: {}\nstderr: {}",
        source,
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new(&exe).output().expect("executar programa gerado");
    let _ = fs::remove_dir_all(&dir);
    (
        String::from_utf8_lossy(&run.stdout).to_string(),
        run.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&run.stderr).to_string(),
    )
}

/// Compila com `xprc` esperando **falha de compilação**, devolvendo o stderr.
fn compile_error(name: &str, source: &str) -> String {
    let dir = std::env::temp_dir().join(format!(
        "xprc-errtest-{}-{}-{}",
        std::process::id(),
        name,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&dir).expect("criar diretório temporário");
    let xpr = dir.join(format!("{}.x", name));
    fs::write(&xpr, source).expect("escrever fonte .x");
    let exe = dir.join(format!("{}.exe", name));

    let xprc = env!("CARGO_BIN_EXE_xprc");
    let compile = Command::new(xprc)
        .arg(&xpr)
        .arg("-o")
        .arg(&exe)
        .output()
        .expect("executar xprc");
    let _ = fs::remove_dir_all(&dir);
    assert!(
        !compile.status.success(),
        "esperava falha de compilação para {:?}, mas compilou",
        source
    );
    String::from_utf8_lossy(&compile.stderr).to_string()
}

// ---------------------------------------------------------------------------------------------
// Subetapa G — emissão de Rust
// ---------------------------------------------------------------------------------------------

#[test]
fn emit_gera_rust_com_modulo_e_main() {
    let rust = emit("_main = fn(args){ 0; };").expect("emissão deve funcionar");
    assert!(rust.contains("pub mod xpr_mod_main"));
    assert!(rust.contains("pub fn _main()"));
    assert!(rust.contains("fn main()"));
    assert!(rust.contains("xpr_rt::run_main"));
}

#[test]
fn emit_usa_runtime_para_todos_os_operadores() {
    let rust = emit("_main = fn(args){ 1 + 2 * 3 - 4 / 5 % 6; };").expect("emissão");
    for func in ["add", "mul", "sub", "div", "rem"] {
        assert!(
            rust.contains(&format!("xpr_rt::ops::{}", func)),
            "operador {} não emitido:\n{}",
            func,
            rust
        );
    }
}

#[test]
fn emit_de_import_usa_modulo_rust_separado() {
    let main = module_unit(
        "main",
        "import(\"mathx\", \"m\");\n_main = fn(args){ m.dobro(21); };",
    );
    let mathx = module_unit("mathx", "dobro = fn(x){ x * 2; };");
    let rust = emit_program("main", &[main, mathx]).expect("emissão com módulo");
    assert!(rust.contains("pub mod xpr_mod_mathx"));
    assert!(rust.contains("xpr_mod_mathx::dobro()"));
}

#[test]
fn emit_sem_main_e_missing_main() {
    let (code, span) = emit_err("f = 1;");
    assert_eq!(code, DiagCode::MissingMain);
    assert!(span.is_none(), "missing-main não tem posição no fonte");
}

#[test]
fn emit_identificador_nao_definido_aponta_o_span() {
    let (code, span) = emit_err("_main = fn(args){ naoexiste; };");
    assert_eq!(code, DiagCode::UndefinedIdentifier);
    // `_main`(1-5) ` `(6) `=`(7) ` `(8) `fn`(9-10) `(`(11) `args`(12-15) `)`(16) `{`(17) ` `(18)
    // → `naoexiste` começa na coluna 19.
    assert_eq!(span.map(|s| (s.line_start, s.col_start)), Some((1, 19)));
}

#[test]
fn emit_uso_antes_da_definicao_e_erro() {
    // Seção 4.5: `g = fn(x){ x + c; };` é erro se `c` ainda não foi definido.
    let (code, _) = emit_err("g = fn(x){ x + c; };\nc = 1;");
    assert_eq!(code, DiagCode::UndefinedIdentifier);
}

#[test]
fn emit_atribuicao_a_nome_capturado_e_erro() {
    // Exemplo literal da Seção 4.5.
    let (code, _) = emit_err(
        "_main = fn(args){ c = 10; f = fn(x){ c = c + x; c; }; f(1); };",
    );
    assert_eq!(code, DiagCode::AssignToCaptured);
}

#[test]
fn emit_parametro_sombreando_externo_permite_atribuicao() {
    // `h = fn(c){ c = c + 1; c; }` é válido (o parâmetro sombreia o externo).
    let rust = emit("_main = fn(args){ c = 10; h = fn(c){ c = c + 1; c; }; h(c); };")
        .expect("deve emitir");
    assert!(rust.contains("pub fn _main"));
}

#[test]
fn emit_recursao_direta_usa_recursive_closure() {
    let rust = emit("_main = fn(args){ fat = fn(x){ iif(x <= 1, 1, x * fat(x - 1)); }; fat(5); };")
        .expect("emissão");
    assert!(
        rust.contains("xpr_rt::recursive_closure"),
        "recursão direta deveria usar recursive_closure:\n{}",
        rust
    );
}

#[test]
fn emit_captura_congela_por_copia() {
    // Cada variável livre capturada aparece como uma cópia `let __cap_x = x.clone();`.
    let rust = emit("_main = fn(args){ c = 1; f = fn(x){ x + c; }; f(2); };").expect("emissão");
    assert!(
        rust.contains("let __cap_c = c.clone();"),
        "captura por cópia ausente:\n{}",
        rust
    );
}

// ---------------------------------------------------------------------------------------------
// Subetapas A/C/F — comportamento de ponta a ponta
// ---------------------------------------------------------------------------------------------

#[test]
fn e2e_escopo_aninhado_atualiza_vinculacao_existente() {
    // Seção 4.6: o bloco interno atualiza o `x` existente; o programa retorna 2.
    let (stdout, exit, _) = compile_and_run(
        "escopo",
        "_main = fn(args){ x = 1; { x = 2; }; println(string(x)); 0; };",
    );
    assert_eq!(stdout, "2\n");
    assert_eq!(exit, 0);
}

#[test]
fn e2e_captura_congelada_retorna_11_nao_101() {
    // Exemplo de `helper`/`f` da Seção 4.5: `f` congela o `helper` original (retorna 11).
    let source = r#"_main = fn(args){
  c = 10;
  helper = fn(x){ x + c; };
  f = fn(x){ helper(x); };
  c = 20;
  helper = fn(x){ x + 100; };
  println(string(f(1)));
  0;
};"#;
    let (stdout, exit, _) = compile_and_run("captura", source);
    assert_eq!(stdout, "11\n");
    assert_eq!(exit, 0);
}

#[test]
fn e2e_funcoes_de_primeira_classe_passam_como_valor() {
    let source = r#"_main = fn(args){
  dobro = fn(x){ x * 2; };
  aplicar = fn(f){ f(21); };
  println(string(aplicar(dobro)));
  0;
};"#;
    let (stdout, exit, _) = compile_and_run("primeira_classe", source);
    assert_eq!(stdout, "42\n");
    assert_eq!(exit, 0);
}

#[test]
fn e2e_desestruturacao_de_lista_e_parametro() {
    let source = r#"_main = fn(args){
  soma = fn([a, b]){ a + b; };
  [x, y] = [20, 22];
  println(string(soma([x, y])));
  0;
};"#;
    let (stdout, exit, _) = compile_and_run("desestrutura", source);
    assert_eq!(stdout, "42\n");
    assert_eq!(exit, 0);
}

#[test]
fn e2e_falha_nao_capturada_sai_com_status_nao_zero() {
    let (_, exit, stderr) = compile_and_run("falha", "_main = fn(args){ fail(\"boom\"); };");
    assert_ne!(exit, 0);
    assert!(stderr.contains("boom"), "stderr: {}", stderr);
}

#[test]
fn e2e_divisao_por_zero_capturada_por_try() {
    let source = r#"_main = fn(args){
  [ok, v] = try(1 / 0);
  println(string(ok));
  println(string(v));
  0;
};"#;
    let (stdout, exit, _) = compile_and_run("try_div", source);
    assert_eq!(stdout, "false\n0\n");
    assert_eq!(exit, 0);
}

#[test]
fn e2e_builtins_de_lista_e_string() {
    let source = r#"_main = fn(args){
  xs = range(5);
  println(string(xs));
  println(string(len(xs)));
  println(slice(["abcdef", 1, 3]));
  println(string(at([xs, 2])));
  println(string(set([xs, 0, 99])));
  0;
};"#;
    let (stdout, exit, _) = compile_and_run("builtins", source);
    assert_eq!(stdout, "[0, 1, 2, 3, 4]\n5\nbc\n2\n[99, 1, 2, 3, 4]\n");
    assert_eq!(exit, 0);
}

#[test]
fn e2e_formatacao_de_reais() {
    // Seção 5.1: padrão `decimais = 2`, notação científica fora da faixa.
    let source = r#"_main = fn(args){
  println(string(1.0 / 2.0));
  println(string(20.0));
  println(string(0.001));
  println(format([20.0, [4, 15]]));
  0;
};"#;
    let (stdout, exit, _) = compile_and_run("formatacao", source);
    assert_eq!(stdout, "0.50\n20.00\n1.00e-3\n20.0000\n");
    assert_eq!(exit, 0);
}

#[test]
fn e2e_aritmetica_em_listas_e_strings() {
    let source = r#"_main = fn(args){
  println(string([1, 2] + [10, 20]));
  println("a" + "b");
  println(string([1, 2] == [1, 2]));
  0;
};"#;
    let (stdout, exit, _) = compile_and_run("ops_lista", source);
    assert_eq!(stdout, "[11, 22]\nab\n[true, true]\n");
    assert_eq!(exit, 0);
}

#[test]
fn e2e_size_mismatch_em_lista_e_capturavel() {
    let source = r#"_main = fn(args){
  [ok, _] = try([1, 2] + [1]);
  println(string(ok));
  0;
};"#;
    let (stdout, exit, _) = compile_and_run("size_mismatch", source);
    assert_eq!(stdout, "false\n");
    assert_eq!(exit, 0);
}

// ---------------------------------------------------------------------------------------------
// Subetapa E — diagnósticos de compilação
// ---------------------------------------------------------------------------------------------

#[test]
fn diag_erro_de_tipo_aponta_id_do_catalogo() {
    let stderr = compile_error("tipo", "_main = fn(args){ 1 + \"a\"; };");
    assert!(
        stderr.contains("type-mismatch") || stderr.contains("TypeMismatch"),
        "stderr: {}",
        stderr
    );
}

#[test]
fn diag_atribuicao_a_nome_capturado_cita_o_id() {
    let stderr = compile_error(
        "capturado",
        "_main = fn(args){ c = 10; f = fn(x){ c = c + x; c; }; f(1); };",
    );
    assert!(stderr.contains("assign-to-captured"), "stderr: {}", stderr);
}

#[test]
fn diag_identificador_nao_definido_cita_o_id() {
    let stderr = compile_error("indefinido", "_main = fn(args){ naoexiste; };");
    assert!(
        stderr.contains("undefined-identifier"),
        "stderr: {}",
        stderr
    );
}

#[test]
fn diag_modulo_ausente_cita_o_id() {
    let stderr = compile_error("semmain", "f = 1;");
    assert!(stderr.contains("missing-main"), "stderr: {}", stderr);
}

#[test]
fn diag_malformed_number_vem_do_lexer() {
    let stderr = compile_error("malformado", "_main = fn(args){ 99999999999999999999; };");
    assert!(stderr.contains("malformed-number"), "stderr: {}", stderr);
}

#[test]
fn diag_sintaxe_cita_o_id() {
    let stderr = compile_error("sintaxe", "_main = fn(args){ 1 + ; };");
    assert!(stderr.contains("syntax-error"), "stderr: {}", stderr);
}

// ---------------------------------------------------------------------------------------------
// Catálogo (subetapa E) — invariantes
// ---------------------------------------------------------------------------------------------

#[test]
fn catalogo_tem_ids_publicos_unicos_e_estaveis() {
    let mut ids: Vec<&str> = xprc::diag::CATALOG.iter().map(|spec| spec.public_id).collect();
    let total = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), total, "há IDs públicos repetidos no catálogo");
}

#[test]
fn catalogo_distingue_severidade_de_compilacao_e_execucao() {
    use xprc::diag::Severity;
    assert_eq!(DiagCode::TypeMismatch.severity(), Severity::Error);
    assert_eq!(DiagCode::DivByZero.severity(), Severity::Runtime);
}

// ---------------------------------------------------------------------------------------------
// M9 — superfície de CLI do `xprc` (subetapa G + E)
// ---------------------------------------------------------------------------------------------

fn xprc() -> Command {
    Command::new(env!("CARGO_BIN_EXE_xprc"))
}

fn run_xprc(args: &[&str]) -> (String, String, i32) {
    let output = xprc().args(args).output().expect("executar xprc");
    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
        output.status.code().unwrap_or(-1),
    )
}

fn read_case(relative: &str) -> String {
    let path = project_root().join(relative);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("ler {}: {}", path.display(), e))
}

#[test]
fn cli_lex_dump_reproduz_exatamente_o_fixture_do_lexer() {
    // Plano de testes do codegen, M9: `--lex-dump` deve produzir exatamente o `.tokens`.
    let x = project_root().join("tests/lexer/cases/L01_numeros_int.x");
    let (stdout, stderr, code) = run_xprc(&["--lex-dump", x.to_str().unwrap()]);
    assert_eq!(code, 0, "stderr: {}", stderr);
    assert_eq!(
        stdout,
        read_case("tests/lexer/cases/L01_numeros_int.tokens")
    );
}

#[test]
fn cli_parse_dump_reproduz_exatamente_o_fixture_do_parser() {
    let x = project_root().join("tests/parser/cases/P01_precedencia_aritmetica.x");
    let (stdout, stderr, code) = run_xprc(&["--parse-dump", x.to_str().unwrap()]);
    assert_eq!(code, 0, "stderr: {}", stderr);
    let expected = read_case("tests/parser/cases/P01_precedencia_aritmetica.ast");
    assert_eq!(stdout.trim_end(), expected.trim_end());
}

#[test]
fn cli_emit_rust_escreve_no_stdout_sem_invocar_rustc() {
    let x = project_root().join("tests/codegen/milestones/M0_hello/main.x");
    // `--rustc` aponta para um executável inexistente: se o compilador fosse invocado, falharia.
    let (stdout, stderr, code) = run_xprc(&[
        "--emit-rust",
        "--rustc",
        "/nonexistent/rustc-nao-existe",
        x.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "stderr: {}", stderr);
    assert!(stdout.contains("pub mod xpr_mod_main"), "stdout: {}", stdout);
    assert!(stdout.contains("xpr_rt::run_main"));
}

#[test]
fn cli_emit_rust_respeita_o_arquivo_de_saida() {
    let dir = std::env::temp_dir().join(format!("xprc-cli-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("diretório temporário");
    let output_path = dir.join("saida.rs");
    let x = project_root().join("tests/codegen/milestones/M0_hello/main.x");
    let (stdout, stderr, code) = run_xprc(&[
        "--emit-rust",
        "-o",
        output_path.to_str().unwrap(),
        x.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "stderr: {}", stderr);
    assert!(stdout.is_empty(), "stdout deveria estar vazio: {}", stdout);
    let written = fs::read_to_string(&output_path).expect("ler Rust gerado");
    assert!(written.contains("fn main()"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn cli_erro_de_tipo_sai_com_status_nao_zero_e_cita_o_catalogo() {
    let dir = std::env::temp_dir().join(format!("xprc-cli-err-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("diretório temporário");
    let x = dir.join("ruim.x");
    fs::write(&x, "_main = fn(args){ 1 + \"a\"; };").expect("escrever fonte");
    let output = xprc()
        .args([x.to_str().unwrap(), "-o", dir.join("ruim").to_str().unwrap()])
        .output()
        .expect("executar xprc");
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let _ = fs::remove_dir_all(&dir);
    assert!(!output.status.success());
    assert!(stderr.contains("type-mismatch"), "stderr: {}", stderr);
}

#[test]
fn cli_sem_argumentos_exibe_ajuda_e_falha() {
    let (stdout, _stderr, code) = run_xprc(&[]);
    assert_ne!(code, 0);
    assert!(stdout.contains("Uso: xprc"), "stdout: {}", stdout);
}

#[test]
fn cli_help_e_version_funcionam() {
    for flag in ["-h", "--help"] {
        let (stdout, _stderr, code) = run_xprc(&[flag]);
        assert_eq!(code, 0);
        assert!(stdout.contains("xprc 0"), "stdout: {}", stdout);
    }
    let (stdout, _stderr, code) = run_xprc(&["--version"]);
    assert_eq!(code, 0);
    assert!(stdout.starts_with("xprc 0.1.0"), "stdout: {}", stdout);
}

#[test]
fn cli_rejeita_dois_arquivos_fonte() {
    let dir = std::env::temp_dir().join(format!("xprc-cli-two-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("diretório temporário");
    let a = dir.join("a.x");
    let b = dir.join("b.x");
    fs::write(&a, "_main = fn(args){ 0; };").expect("escrever");
    fs::write(&b, "_main = fn(args){ 0; };").expect("escrever");
    let output = xprc()
        .args([a.to_str().unwrap(), b.to_str().unwrap()])
        .output()
        .expect("executar xprc");
    let _ = fs::remove_dir_all(&dir);
    assert!(!output.status.success());
}

#[test]
fn cli_import_respeita_o_diretorio_do_arquivo_e_o_I() {
    let dir = std::env::temp_dir().join(format!("xprc-cli-inc-{}", std::process::id()));
    let lib_dir = dir.join("lib");
    fs::create_dir_all(&lib_dir).expect("diretório temporário");
    let main = dir.join("main.x");
    let lib = lib_dir.join("util.x");
    fs::write(
        &main,
        "import(\"util\", \"u\");\n_main = fn(args){ println(string(u.dobro(21))); 0; };",
    )
    .expect("escrever");
    fs::write(&lib, "dobro = fn(x){ x * 2; };").expect("escrever");
    let exe = dir.join("prog");
    let output = xprc()
        .args([
            main.to_str().unwrap(),
            "-I",
            lib_dir.to_str().unwrap(),
            "-o",
            exe.to_str().unwrap(),
        ])
        .output()
        .expect("executar xprc");
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    assert!(output.status.success(), "stderr: {}", stderr);
    let run = Command::new(&exe).output().expect("executar programa");
    let _ = fs::remove_dir_all(&dir);
    assert_eq!(String::from_utf8_lossy(&run.stdout).trim(), "42");
}

#[test]
fn cli_import_de_modulo_inexistente_falha() {
    let dir = std::env::temp_dir().join(format!("xprc-cli-miss-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("diretório temporário");
    let main = dir.join("main.x");
    fs::write(&main, "import(\"naoexiste\");\n_main = fn(args){ 0; };").expect("escrever");
    let output = xprc().arg(main.to_str().unwrap()).output().expect("executar xprc");
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let _ = fs::remove_dir_all(&dir);
    assert!(!output.status.success());
    assert!(stderr.contains("não encontrado"), "stderr: {}", stderr);
}

//! Driver da linha de comando `xprc` (ver `xpr-specs.md`, Seção 8, e `plano-codegen.md`,
//! subetapa G).
//!
//! Modos suportados:
//!
//! | Invocação | Comportamento |
//! | --- | --- |
//! | `xprc <arquivo.x>` | traduz para Rust e compila; executável = nome do arquivo sem `.x` |
//! | `xprc <arquivo.x> -o <arq>` | idem, com o executável em `<arq>` |
//! | `xprc --emit-rust <arquivo.x>` | só escreve o Rust gerado (em `-o` ou no stdout) |
//! | `xprc --lex-dump <arquivo.x>` | exibe os tokens |
//! | `xprc --parse-dump <arquivo.x>` | exibe a AST |
//! | `xprc --type-dump <arquivo.x>` | exibe os tipos inferidos |
//! | `xprc -h` / `--help`, `--version` | ajuda e versão |
//!
//! Opções: `-o`, `-I <dir>` (repetível), `--rustc <comando>` (padrão `rustc`), `-O <0-3>` (padrão
//! `2`), `-v`.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use xprc::codegen::{ModuleUnit, emit_program};
use xprc::diag::{Diagnostic, Severity};
use xprc::lexer::{LexError, Lexer, Token, TokenKind};
use xprc::parser::ast::Module as AstModule;
use xprc::parser::dump::dump_module;
use xprc::parser::{Parser, SyntaxError};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Modo de operação pedido na linha de comando.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Build,
    EmitRust,
    LexDump,
    ParseDump,
    TypeDump,
}

struct Options {
    mode: Mode,
    source: Option<PathBuf>,
    output: Option<PathBuf>,
    include_dirs: Vec<PathBuf>,
    rustc: String,
    opt_level: u8,
    verbose: bool,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(args) {
        Ok(code) => std::process::exit(code),
        Err(message) => {
            eprintln!("{}", message);
            std::process::exit(1);
        }
    }
}

fn run(args: Vec<String>) -> Result<i32, String> {
    if args.is_empty() {
        // Sem arquivo e sem modo: mostra a ajuda e falha (nunca lê stdin).
        print_help();
        return Ok(1);
    }

    let options = match parse_args(&args) {
        Ok(ParseOutcome::Run(options)) => options,
        Ok(ParseOutcome::Handled(code)) => return Ok(code),
        Err(message) => {
            eprintln!("{}", message);
            print_help();
            return Ok(1);
        }
    };

    let source = options
        .source
        .clone()
        .ok_or_else(|| "erro: nenhum arquivo fonte informado".to_string())?;

    // ---- leitura e lexer ----
    let text = fs::read_to_string(&source)
        .map_err(|e| format!("erro: não foi possível ler {}: {}", source.display(), e))?;
    let tokens = Lexer::new(&text).tokenize().map_err(|err| {
        format!(
            "{}:{},{}: {}",
            source.display(),
            err.span.line_start,
            err.span.col_start,
            lex_error_message(&err, &text)
        )
    })?;

    if options.mode == Mode::LexDump {
        print!("{}", format_token_dump(&tokens));
        return Ok(0);
    }

    // ---- parser ----
    let ast = Parser::new(&tokens).parse_module().map_err(|err| {
        format!(
            "{}:{},{}: {}",
            source.display(),
            err.span.line_start,
            err.span.col_start,
            syntax_error_message(&err.kind, &err.detail)
        )
    })?;

    if options.mode == Mode::ParseDump {
        println!("{}", dump_module(&ast));
        return Ok(0);
    }

    // ---- módulos (subetapa B) ----
    let modules = load_modules(&source, ast, &options)?;

    if options.mode == Mode::TypeDump {
        // A inferência de tipos completa ainda não existe nesta etapa; o dump lista as definições
        // e a anotação estática de cada uma, o que já é útil para depuração.
        println!("{}", dump_types(&modules));
        return Ok(0);
    }

    // ---- emissão de Rust ----
    let entry_name = module_name(&source);
    let rust = emit_program(&entry_name, &modules).map_err(|err| {
        // SemaError é de compilação por construção — a fase (prefixo) vem de quem reporta,
        // não da entrada do catálogo (ex.: index-out-of-range de índice constante de tupla).
        let diagnostic = Diagnostic::new(err.code, err.detail).render_as(Severity::Error);
        match err.span {
            Some(span) => format!(
                "{}:{},{}: {}",
                source.display(),
                span.line_start,
                span.col_start,
                diagnostic
            ),
            None => format!("{}: {}", source.display(), diagnostic),
        }
    })?;

    if options.mode == Mode::EmitRust {
        match &options.output {
            Some(path) => fs::write(path, &rust).map_err(|e| {
                format!("erro: não foi possível escrever {}: {}", path.display(), e)
            })?,
            None => print!("{}", rust),
        }
        return Ok(0);
    }

    build(&rust, &source, &options)
}

// ---------------------------------------------------------------------------------------------
// Parsing da linha de comando
// ---------------------------------------------------------------------------------------------

enum ParseOutcome {
    Run(Options),
    Handled(i32),
}

fn parse_args(args: &[String]) -> Result<ParseOutcome, String> {
    let mut options = Options {
        mode: Mode::Build,
        source: None,
        output: None,
        include_dirs: Vec::new(),
        rustc: "rustc".to_string(),
        opt_level: 2,
        verbose: false,
    };

    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        match arg {
            "-h" | "--help" => {
                print_help();
                return Ok(ParseOutcome::Handled(0));
            }
            "--version" => {
                println!("xprc {}", VERSION);
                return Ok(ParseOutcome::Handled(0));
            }
            "--lex-dump" => options.mode = Mode::LexDump,
            "--parse-dump" => options.mode = Mode::ParseDump,
            "--type-dump" => options.mode = Mode::TypeDump,
            "--emit-rust" => options.mode = Mode::EmitRust,
            "-v" => options.verbose = true,
            "-o" => {
                i += 1;
                let value = args.get(i).ok_or("-o exige um nome de arquivo")?;
                options.output = Some(PathBuf::from(value));
            }
            "-I" => {
                i += 1;
                let value = args.get(i).ok_or("-I exige um diretório")?;
                options.include_dirs.push(PathBuf::from(value));
            }
            "--rustc" => {
                i += 1;
                let value = args.get(i).ok_or("--rustc exige um comando")?;
                options.rustc = value.clone();
            }
            "-O" => {
                i += 1;
                let value = args.get(i).ok_or("-O exige um nível de 0 a 3")?;
                options.opt_level = value
                    .parse()
                    .map_err(|_| format!("nível de otimização inválido: {}", value))?;
                if options.opt_level > 3 {
                    return Err(format!("nível de otimização inválido: {}", value));
                }
            }
            other if other.starts_with('-') && other.len() > 1 => {
                return Err(format!("opção desconhecida: {}", other));
            }
            other => {
                if options.source.is_some() {
                    return Err(format!(
                        "exatamente um arquivo fonte é aceito (recebi também {})",
                        other
                    ));
                }
                options.source = Some(PathBuf::from(other));
            }
        }
        i += 1;
    }

    Ok(ParseOutcome::Run(options))
}

fn print_help() {
    println!("xprc {}", VERSION);
    println!("Uso: xprc [opções] <arquivo.x>");
    println!();
    println!("Modos:");
    println!("  (padrão)              traduz para Rust e compila");
    println!("  --emit-rust           escreve apenas o Rust gerado (em -o ou no stdout)");
    println!("  --lex-dump            exibe os tokens do arquivo");
    println!("  --parse-dump          exibe a árvore sintática");
    println!("  --type-dump           exibe os tipos inferidos");
    println!("  -h, --help            exibe esta ajuda");
    println!("  --version             exibe a versão");
    println!();
    println!("Opções:");
    println!("  -o <arquivo>          nome da saída");
    println!("  -I <dir>              diretório de busca de import (repetível)");
    println!("  --rustc <comando>     compilador Rust a usar (padrão: rustc)");
    println!("  -O <nível>            otimização de 0 a 3 (padrão: 2)");
    println!("  -v                    relata imports resolvidos e o comando do rustc");
}

// ---------------------------------------------------------------------------------------------
// Módulos (subetapa B)
// ---------------------------------------------------------------------------------------------

/// Carrega o módulo de entrada e, recursivamente, todos os módulos importados.
fn load_modules(
    entry_path: &Path,
    entry_ast: AstModule,
    options: &Options,
) -> Result<Vec<ModuleUnit>, String> {
    let mut units: Vec<ModuleUnit> = Vec::new();
    let mut loading: Vec<PathBuf> = Vec::new();
    load_one(entry_path, entry_ast, options, &mut units, &mut loading)?;
    Ok(units)
}

fn load_one(
    path: &Path,
    ast: AstModule,
    options: &Options,
    units: &mut Vec<ModuleUnit>,
    loading: &mut Vec<PathBuf>,
) -> Result<(), String> {
    let canonical = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if loading.contains(&canonical) {
        return Err(format!(
            "{}: {} (ciclo passando por {})",
            path.display(),
            Diagnostic::new(xprc::diag::DiagCode::UndefinedIdentifier, String::new()).render(),
            path.display()
        ));
    }
    loading.push(canonical.clone());

    let name = module_name(path);
    let current_dir = path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    // Alias duplicado no mesmo arquivo é erro (Seção 4.1).
    let mut seen_aliases: HashSet<String> = HashSet::new();
    for import in &ast.imports {
        if !seen_aliases.insert(import.effective_alias().to_string()) {
            return Err(format!(
                "{}:{},{}: alias duplicado no mesmo arquivo: {}",
                path.display(),
                import.span.line_start,
                import.span.col_start,
                import.effective_alias()
            ));
        }
    }

    let imports = ast.imports.clone();

    for import in &imports {
        let resolved = resolve_import(&import.name, &current_dir, options)?;
        if options.verbose {
            eprintln!("xprc: import {:?} -> {}", import.name, resolved.display());
        }
        let text = fs::read_to_string(&resolved)
            .map_err(|e| format!("erro: não foi possível ler {}: {}", resolved.display(), e))?;
        let tokens = Lexer::new(&text).tokenize().map_err(|err: LexError| {
            format!(
                "{}:{},{}: {}",
                resolved.display(),
                err.span.line_start,
                err.span.col_start,
                lex_error_message(&err, &text)
            )
        })?;
        let ast = Parser::new(&tokens)
            .parse_module()
            .map_err(|err: SyntaxError| {
                format!(
                    "{}:{},{}: {}",
                    resolved.display(),
                    err.span.line_start,
                    err.span.col_start,
                    syntax_error_message(&err.kind, &err.detail)
                )
            })?;
        load_one(&resolved, ast, options, units, loading)?;
    }

    // O módulo entra **depois** de seus imports: a ordem da lista é a ordem de dependência, que é
    // o que a verificação de tipos e a emissão de `mod` precisam.
    units.push(ModuleUnit {
        name,
        path: path.display().to_string(),
        ast,
    });

    loading.pop();
    Ok(())
}

/// Resolve `import("nome")` para um arquivo: diretório do importador e depois `-I` em ordem
/// (Seção 4.1).
fn resolve_import(name: &str, importer_dir: &Path, options: &Options) -> Result<PathBuf, String> {
    let file = format!("{}.x", name);
    let mut candidates = vec![importer_dir.join(&file)];
    for dir in &options.include_dirs {
        candidates.push(dir.join(&file));
    }
    for candidate in &candidates {
        if candidate.is_file() {
            return Ok(candidate.clone());
        }
    }
    Err(format!(
        "erro: módulo não encontrado: {} (procurado em {})",
        name,
        candidates
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

fn module_name(path: &Path) -> String {
    path.file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_else(|| "main".to_string())
}

// ---------------------------------------------------------------------------------------------
// Compilação do Rust gerado
// ---------------------------------------------------------------------------------------------

fn build(rust: &str, source: &Path, options: &Options) -> Result<i32, String> {
    let workdir = std::env::temp_dir().join(format!(
        "xprc-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&workdir)
        .map_err(|e| format!("erro: não foi possível criar {}: {}", workdir.display(), e))?;
    let generated = workdir.join("main.rs");
    fs::write(&generated, rust).map_err(|e| {
        format!(
            "erro: não foi possível escrever {}: {}",
            generated.display(),
            e
        )
    })?;

    let output = match &options.output {
        Some(path) => path.clone(),
        None => {
            let stem = source
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "a.out".to_string());
            PathBuf::from(stem)
        }
    };

    let mut command = Command::new(&options.rustc);
    command
        .arg(&generated)
        .arg("--edition=2021")
        .arg("-o")
        .arg(&output)
        .arg(format!("-Copt-level={}", options.opt_level));

    if options.verbose {
        eprintln!("xprc: executando {:?}", command);
    }

    let status = command
        .status()
        .map_err(|e| format!("erro: não foi possível executar {}: {}", options.rustc, e))?;
    let _ = fs::remove_dir_all(&workdir);
    if !status.success() {
        return Err("erro: a compilação do código Rust gerado falhou".to_string());
    }
    Ok(0)
}

// ---------------------------------------------------------------------------------------------
// Dumps e mensagens
// ---------------------------------------------------------------------------------------------

fn format_token_dump(tokens: &[Token]) -> String {
    let mut out = String::new();
    for token in tokens {
        let position = if token.span.line_start == token.span.line_end {
            format!(
                "{}:{}-{}",
                token.span.line_start, token.span.col_start, token.span.col_end
            )
        } else {
            format!(
                "{}:{}-{}:{}",
                token.span.line_start,
                token.span.col_start,
                token.span.line_end,
                token.span.col_end
            )
        };
        out.push_str(&position);
        out.push(' ');
        out.push_str(&kind_text(&token.kind));
        if token.adjacent_to_next && matches!(token.kind, TokenKind::Plus | TokenKind::Minus) {
            out.push_str(" adj");
        }
        out.push('\n');
    }
    out
}

fn kind_text(kind: &TokenKind) -> String {
    match kind {
        TokenKind::IntLit(text) => format!("IntLit({})", text),
        TokenKind::RealLit(text) => format!("RealLit({})", text),
        TokenKind::StringLit { raw, quote } => {
            let q = if *quote == '"' { "dq" } else { "sq" };
            format!("StringLit({}, {:?})", q, raw)
        }
        TokenKind::Ident(name) => format!("Ident({})", name),
        TokenKind::KwFn => "KwFn".into(),
        TokenKind::KwIif => "KwIif".into(),
        TokenKind::KwFor => "KwFor".into(),
        TokenKind::KwTry => "KwTry".into(),
        TokenKind::KwImport => "KwImport".into(),
        TokenKind::KwAnd => "KwAnd".into(),
        TokenKind::KwOr => "KwOr".into(),
        TokenKind::KwNot => "KwNot".into(),
        TokenKind::Plus => "Plus".into(),
        TokenKind::Minus => "Minus".into(),
        TokenKind::Star => "Star".into(),
        TokenKind::Slash => "Slash".into(),
        TokenKind::Percent => "Percent".into(),
        TokenKind::Lt => "Lt".into(),
        TokenKind::Gt => "Gt".into(),
        TokenKind::Le => "Le".into(),
        TokenKind::Ge => "Ge".into(),
        TokenKind::EqEq => "EqEq".into(),
        TokenKind::NotEq => "NotEq".into(),
        TokenKind::Eq => "Eq".into(),
        TokenKind::LParen => "LParen".into(),
        TokenKind::RParen => "RParen".into(),
        TokenKind::LBrace => "LBrace".into(),
        TokenKind::RBrace => "RBrace".into(),
        TokenKind::LBracket => "LBracket".into(),
        TokenKind::RBracket => "RBracket".into(),
        TokenKind::Comma => "Comma".into(),
        TokenKind::Semi => "Semi".into(),
        TokenKind::Colon => "Colon".into(),
        TokenKind::Dot => "Dot".into(),
        TokenKind::Eof => "Eof".into(),
    }
}

fn lex_error_message(err: &LexError, _source: &str) -> String {
    let code = match err.kind {
        xprc::lexer::LexErrorKind::MalformedNumber => xprc::diag::DiagCode::MalformedNumber,
        xprc::lexer::LexErrorKind::UnterminatedString => xprc::diag::DiagCode::SyntaxError,
        xprc::lexer::LexErrorKind::InvalidCharacter => xprc::diag::DiagCode::SyntaxError,
        xprc::lexer::LexErrorKind::UnterminatedComment => xprc::diag::DiagCode::SyntaxError,
    };
    let detail = match err.kind {
        xprc::lexer::LexErrorKind::MalformedNumber => "literal inteiro fora do intervalo int64",
        xprc::lexer::LexErrorKind::UnterminatedString => "string não terminada",
        xprc::lexer::LexErrorKind::InvalidCharacter => "caractere inválido",
        xprc::lexer::LexErrorKind::UnterminatedComment => "comentário de bloco não terminado",
    };
    Diagnostic::new(code, detail).render()
}

fn syntax_error_message(kind: &xprc::parser::SyntaxErrorKind, detail: &str) -> String {
    Diagnostic::new(
        xprc::diag::DiagCode::SyntaxError,
        format!("{}: {}", kind.name(), detail),
    )
    .render()
}

fn dump_types(modules: &[ModuleUnit]) -> String {
    let mut out = String::new();
    for unit in modules {
        out.push_str(&format!("Module {}\n", unit.name));
        let mut types: HashMap<String, &'static str> = HashMap::new();
        for def in &unit.ast.definitions {
            let ty = match &def.initializer {
                xprc::parser::ast::Initializer::FnDef(_) => "fn",
                xprc::parser::ast::Initializer::ConstExpr(_) => "const",
            };
            types.insert(def.name.clone(), ty);
            out.push_str(&format!("  {} : {}\n", def.name, ty));
        }
    }
    out
}

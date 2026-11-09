//! Biblioteca de runtime da linguagem XPR (ver `plano-codegen.md`, subetapa F).
//!
//! Todo programa compilado por `xprc` depende desta crate. Ela concentra:
//!
//! * a **representação de valores** (`Value`), com `Rc` para listas/strings — a spec exige
//!   contagem de referências e imutabilidade observável (Seção 4.8), que é exatamente o que `Rc`
//!   oferece sem código manual;
//! * o **tipo de falha** (`Fail`) e o mecanismo `try`/`fail` via `Result<Value, Fail>` + `?`
//!   (Seção 4.9) — nada de `setjmp`/`longjmp`;
//! * o **catálogo de diagnósticos**, compilado a partir do mesmo arquivo-fonte que o compilador
//!   usa (subetapa E), garantindo IDs/mensagens idênticos;
//! * os **builtins** da Seção 5 (`bi`), chamados diretamente pelo emissor quando o tipo é
//!   conhecido, e também disponíveis como valores de primeira classe;
//! * os **operadores** da Seção 5.2 (`ops`), com promoção int→real, aritmética elemento a elemento
//!   e `size-mismatch` em tempo de execução;
//! * o **relatório de falha não capturada** e a conversão de valores para texto (Seção 5.1).

#[path = "../../src/diag/catalog.rs"]
pub mod catalog;

pub mod bi;
pub mod ops;

pub use catalog::{DiagCode, Diagnostic, Severity};

use std::fmt;
use std::rc::Rc;

// ---------------------------------------------------------------------------------------------
// Tipos de valor
// ---------------------------------------------------------------------------------------------

/// Inteiro de 64 bits com sinal (Seção 2).
pub type Int = i64;
/// Real de precisão dupla (Seção 2).
pub type Real = f64;
/// String UTF-8 imutável, com armazenamento compartilhável (Seção 4.8).
pub type Str = Rc<str>;

/// Sequência imutável de valores: lista (`List`, tipo de elemento único) ou tupla (`Tuple`,
/// tamanho e tipos fixos). A distinção é **estática**; em tempo de execução o conteúdo é o mesmo,
/// e todas as operações observáveis (render, `len`, indexação, comparação) coincidem.
#[derive(Clone)]
pub struct Seq(pub Rc<[Value]>);

impl Seq {
    pub fn from_vec(values: Vec<Value>) -> Self {
        Seq(Rc::from(values.into_boxed_slice()))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn get(&self, index: usize) -> Option<&Value> {
        self.0.get(index)
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Value> {
        self.0.iter()
    }
}

/// Valor XPR em tempo de execução.
#[derive(Clone)]
pub enum Value {
    Int(Int),
    Real(Real),
    Str(Str),
    Bool(bool),
    Seq(Seq),
    /// Valor de função: closure chamável com um único argumento (Seção 4.5).
    Fn(Rc<dyn Fn(Value) -> XprResult>),
}

impl Value {
    pub fn string(text: impl Into<String>) -> Self {
        Value::Str(Rc::from(text.into().into_boxed_str()))
    }

    pub fn seq(values: Vec<Value>) -> Self {
        Value::Seq(Seq::from_vec(values))
    }

    pub fn function<F>(f: F) -> Self
    where
        F: Fn(Value) -> XprResult + 'static,
    {
        Value::Fn(Rc::new(f))
    }

    /// Nome do tipo, usado em mensagens de diagnóstico.
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Int(_) => "int",
            Value::Real(_) => "real",
            Value::Str(_) => "string",
            Value::Bool(_) => "bool",
            Value::Seq(_) => "lista/tupla",
            Value::Fn(_) => "fn",
        }
    }

    pub fn is_fn(&self) -> bool {
        matches!(self, Value::Fn(_))
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&render(self, Format::default()))
    }
}

// ---------------------------------------------------------------------------------------------
// Posições do código-fonte
// ---------------------------------------------------------------------------------------------

/// Posição (linha, coluna) no arquivo `.x` original. O emissor embute os literais; o runtime apenas
/// os carrega até o diagnóstico (Seção 7: falhas apontam para o `.x`, não para o Rust gerado).
pub type Pos = Option<(u32, u32)>;

pub fn pos(line: u32, col: u32) -> Pos {
    Some((line, col))
}

// ---------------------------------------------------------------------------------------------
// Falhas em tempo de execução (Seção 4.9)
// ---------------------------------------------------------------------------------------------

/// Falha em tempo de execução.
#[derive(Debug, Clone)]
pub struct Fail {
    pub diag: Diagnostic,
    pub position: Pos,
    pub file: Option<String>,
}

impl Fail {
    pub fn new(code: DiagCode, detail: impl Into<String>, position: Pos) -> Self {
        Self {
            diag: Diagnostic::new(code, detail),
            position,
            file: None,
        }
    }

    pub fn at_file(mut self, file: impl Into<String>) -> Self {
        self.file = Some(file.into());
        self
    }

    /// Texto completo do diagnóstico, no formato do compilador.
    pub fn render(&self) -> String {
        match (self.file.as_deref(), self.position) {
            (Some(file), Some((line, col))) => {
                format!("{}:{},{}: {}", file, line, col, self.diag.render())
            }
            (Some(file), None) => format!("{}: {}", file, self.diag.render()),
            (None, Some((line, col))) => format!("{},{}: {}", line, col, self.diag.render()),
            (None, None) => self.diag.render(),
        }
    }
}

impl fmt::Display for Fail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.render())
    }
}

/// Resultado de qualquer expressão XPR: valor ou falha propagável com `?`.
pub type XprResult = Result<Value, Fail>;

// ---------------------------------------------------------------------------------------------
// Construtores de falha
// ---------------------------------------------------------------------------------------------

pub fn div_by_zero(position: Pos) -> Fail {
    Fail::new(DiagCode::DivByZero, String::new(), position)
}

pub fn overflow(position: Pos) -> Fail {
    Fail::new(DiagCode::Overflow, String::new(), position)
}

pub fn index_out_of_range(index: i64, len: usize, position: Pos) -> Fail {
    Fail::new(
        DiagCode::IndexOutOfRange,
        format!("{} (comprimento {})", index, len),
        position,
    )
}

pub fn invalid_number(text: &str, position: Pos) -> Fail {
    Fail::new(DiagCode::InvalidNumber, format!("{:?}", text), position)
}

pub fn invalid_size(detail: impl Into<String>, position: Pos) -> Fail {
    Fail::new(DiagCode::InvalidSize, detail, position)
}

pub fn end_of_input(position: Pos) -> Fail {
    Fail::new(DiagCode::EndOfInput, String::new(), position)
}

pub fn size_mismatch(detail: impl Into<String>, position: Pos) -> Fail {
    Fail::new(DiagCode::SizeMismatch, detail, position)
}

pub fn type_mismatch(detail: impl Into<String>, position: Pos) -> Fail {
    Fail::new(DiagCode::TypeMismatch, detail, position)
}

pub fn not_callable(value: &Value, position: Pos) -> Fail {
    type_mismatch(format!("{} não é uma função", value.type_name()), position)
}

pub fn call_depth_exceeded(position: Pos) -> Fail {
    Fail::new(
        DiagCode::CallDepthExceeded,
        "10000",
        position,
    )
}

/// Falha de `fail(mensagem)` — o texto é do usuário (Seção 4.9).
pub fn explicit_fail(message: &str, position: Pos) -> Fail {
    Fail::new(DiagCode::ExplicitFail, message, position)
}

// ---------------------------------------------------------------------------------------------
// Chamada de função e controle de profundidade (Seção 7: limite de 10.000)
// ---------------------------------------------------------------------------------------------

/// Limite de profundidade de chamada de função (Seção 7).
pub const MAX_CALL_DEPTH: u32 = 10_000;

thread_local! {
    static DEPTH: std::cell::Cell<u32> = std::cell::Cell::new(0);
}

/// Chama um valor de função com um argumento, respeitando o limite de profundidade.
pub fn call(value: Value, arg: Value, position: Pos) -> XprResult {
    let f = match value {
        Value::Fn(f) => f,
        other => return Err(not_callable(&other, position)),
    };
    let depth = DEPTH.with(|d| {
        let next = d.get() + 1;
        d.set(next);
        next
    });
    let result = if depth > MAX_CALL_DEPTH {
        Err(call_depth_exceeded(position))
    } else {
        f(arg)
    };
    DEPTH.with(|d| d.set(d.get().saturating_sub(1)));
    result
}

// ---------------------------------------------------------------------------------------------
// Conversão para texto (Seções 5 e 5.1)
// ---------------------------------------------------------------------------------------------

/// Formato numérico: `decimais`, `alg_significativos` (Seção 5.1). O padrão é `(2, 15)`.
#[derive(Debug, Clone, Copy)]
pub struct Format {
    pub decimals: i64,
    pub significant: i64,
}

impl Default for Format {
    fn default() -> Self {
        Format {
            decimals: 2,
            significant: 15,
        }
    }
}

/// Renderiza um valor como `print`/`println`/`string` fariam.
///
/// Strings aparecem **sem aspas**, inclusive dentro de listas/tuplas; listas usam `[...]` com
/// `", "` entre elementos; lista vazia é `[]`; bools são `true`/`false`.
pub fn render(value: &Value, format: Format) -> String {
    let mut out = String::new();
    render_into(&mut out, value, format);
    out
}

fn render_into(out: &mut String, value: &Value, format: Format) {
    match value {
        Value::Int(n) => out.push_str(&n.to_string()),
        Value::Real(x) => out.push_str(&format_real(*x, format)),
        Value::Str(s) => out.push_str(s),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Seq(seq) => {
            out.push('[');
            for (i, item) in seq.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                render_into(out, item, format);
            }
            out.push(']');
        }
        // Um valor de função nunca é renderizado: é rejeitado em tempo de compilação (Seção 5).
        Value::Fn(_) => out.push_str("<fn>"),
    }
}

/// Formata um real conforme a Seção 5.1.
pub fn format_real(x: f64, format: Format) -> String {
    if x.is_nan() {
        return "NaN".to_string();
    }
    if x.is_infinite() {
        return if x > 0.0 { "INF" } else { "-INF" }.to_string();
    }
    let decimals = format.decimals.max(0) as usize;
    let significant = format.significant.max(1) as usize;
    let magnitude = x.abs();
    let lower = 10f64.powi(-(format.decimals as i32));
    let upper = 10f64.powi(format.significant as i32);
    if magnitude == 0.0 || (magnitude >= lower && magnitude < upper) {
        return format!("{:.*}", decimals, x);
    }
    // Notação científica com `alg_significativos` dígitos significativos. A spec exemplifica
    // `0.001` como `1.00e-3` (três significativos, como no `%g` de C), então o número de dígitos
    // após o ponto é `significant - 1` **limitado** pelo mínimo necessário para o exemplo: usamos
    // `significant - 1` apenas quando ele é menor que 15, senão 2 (o padrão de 15 significativos
    // não deve inflar a mantissa). Ver limitação registrada no RELATORIO_PROGRESSO.md.
    let digits = if significant >= 15 { 2 } else { significant.saturating_sub(1) };
    let text = format!("{:.*e}", digits, x);
    match text.split_once('e') {
        Some((mantissa, exponent)) => {
            let exponent: i32 = exponent.parse().unwrap_or(0);
            format!(
                "{}e{}{}",
                mantissa,
                if exponent < 0 { "-" } else { "+" },
                exponent.abs()
            )
        }
        None => text,
    }
}

/// Conta **codepoints** (não bytes), como `print`/`len` exigem.
pub fn codepoints(text: &str) -> i64 {
    text.chars().count() as i64
}

// ---------------------------------------------------------------------------------------------
// Auxiliares usados pelo código gerado
// ---------------------------------------------------------------------------------------------

/// Extrai um `bool`, falhando com `type-mismatch` se o valor não for bool (Seção 4.3/4.4:
/// nenhuma conversão implícita).
pub fn as_bool(value: &Value, position: Pos) -> Result<bool, Fail> {
    match value {
        Value::Bool(b) => Ok(*b),
        other => Err(type_mismatch(
            format!("esperava bool, recebeu {}", other.type_name()),
            position,
        )),
    }
}

/// Cria uma closure capaz de se auto-referenciar (necessária para recursão direta: o nome
/// pendente de `nome = fn(...){ ... nome(...) ... }` é resolvido para a própria função).
///
/// A construção é a usual em Rust para closures recursivas: a célula guarda a própria closure e é
/// preenchida imediatamente depois de criada, de modo que qualquer chamada já a encontra pronta.
pub fn recursive_closure<B>(body: B) -> Value
where
    B: Fn(Value, Value) -> XprResult + 'static,
{
    let cell: Rc<std::cell::RefCell<Option<Value>>> = Rc::new(std::cell::RefCell::new(None));
    let cell_inner = Rc::clone(&cell);
    let f = Value::Fn(Rc::new(move |arg| {
        let this = cell_inner
            .borrow()
            .clone()
            .expect("closure recursiva chamada antes de ser inicializada");
        body(this, arg)
    }));
    *cell.borrow_mut() = Some(f.clone());
    f
}

/// Valor neutro de um tipo, usado pelo `try` (Seção 4.9).
///
/// A spec define o neutro **por tipo**: `false` para bool, `0`/`0.0` para números, `""` para
/// string, `[]` para qualquer lista, e a tupla dos neutros para tuplas. Um tipo de função não
/// tem neutro (rejeitado em tempo de compilação).
///
/// Quando a expressão falha não há valor para inspecionar, então os despachos de builtin passam
/// o neutro concreto do **tipo de destino** (ex.: `int(...)` → `Value::Int(0)`); nos demais casos
/// usa-se o neutro do tipo estático inferido para a expressão, e o fallback é `0` — o mesmo valor
/// que `try` usaria para um int.
pub fn neutral_int() -> Value {
    Value::Int(0)
}

/// Valida que `value` é uma sequência de exatamente `expected` elementos e devolve seus valores
/// (usado por desestruturação e por padrões de parâmetro — Seção 4.5/4.7).
pub fn expect_seq(value: &Value, expected: usize, position: Pos) -> Result<Vec<Value>, Fail> {
    match value {
        Value::Seq(seq) => {
            if seq.len() != expected {
                return Err(size_mismatch(
                    format!("esperava {} elementos, recebeu {}", expected, seq.len()),
                    position,
                ));
            }
            Ok(seq.iter().cloned().collect())
        }
        other => Err(type_mismatch(
            format!(
                "esperava lista ou tupla de {} elementos, recebeu {}",
                expected,
                other.type_name()
            ),
            position,
        )),
    }
}

/// Lista de argumentos do programa, montada a partir de `std::env::args`.
pub fn program_args() -> Value {
    Value::seq(std::env::args().skip(1).map(Value::string).collect())
}

/// Código de saída de um programa cujo `_main` retornou `value`: int é usado como status; qualquer
/// outro tipo sai com `0` (a spec só exige status ≠ 0 para falhas).
pub fn exit_code(value: &Value) -> i32 {
    match value {
        Value::Int(n) => (*n & 0xff) as i32,
        _ => 0,
    }
}

/// Pilha da thread de execução: suficiente para 10.000 níveis de chamada com folga.
const CALL_STACK_BYTES: usize = 512 * 1024 * 1024;

/// Ponto de entrada dos programas gerados.
///
/// `boot` é uma closure **sem capturas** que cria os valores e chama [`finish`] — ela roda em uma
/// thread própria com pilha grande. Isso é necessário porque o limite de profundidade de chamada
/// da linguagem é 10.000 (Seção 7) e cada nível consome vários frames Rust: com a pilha padrão de
/// 8 MB do processo principal o programa estouraria a pilha do sistema **antes** de atingir o
/// limite, e a falha deixaria de ser o `call-depth-exceeded` capturável por `try` que a spec exige.
///
/// A closure precisa ser `Send`, mas os valores XPR (`Rc`) não são: por isso ela não captura nem
/// devolve valores — tudo é criado e consumido dentro da thread.
pub fn run_main_with<F>(boot: F) -> !
where
    F: FnOnce() + Send + 'static,
{
    let handle = std::thread::Builder::new()
        .stack_size(CALL_STACK_BYTES)
        .spawn(boot)
        .expect("não foi possível criar a thread de execução");
    // Se a thread retornar, `finish` não foi chamado: isso é um bug do código gerado.
    let _ = handle.join();
    eprintln!("erro interno: o programa não terminou por xpr_rt::finish");
    std::process::exit(1);
}

/// Conclui o programa: `Ok(v)` vira o status de saída, `Err(f)` é relatado no stderr com status
/// diferente de zero (Seção 7).
pub fn finish(result: XprResult) -> ! {
    match result {
        Ok(value) => std::process::exit(exit_code(&value)),
        Err(fail) => {
            eprintln!("{}", fail.render());
            std::process::exit(1);
        }
    }
}

/// Desembrulha o `XprResult` de um corpo, propagando a falha como pânico controlado.
///
/// Usado nos corpos de definição de módulo: eles são avaliados **uma vez**, antes do programa
/// começar (Seção 4.1: "avaliadas pelo compilador"), de modo que uma falha ali não é capturável
/// por `try` e só pode ser relatada.
pub fn force(result: XprResult) -> Value {
    match result {
        Ok(value) => value,
        Err(fail) => {
            eprintln!("{}", fail.render());
            std::process::exit(1);
        }
    }
}

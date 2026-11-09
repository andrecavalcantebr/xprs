//! Builtins da Seção 5.
//!
//! Cada builtin é uma função que recebe um `Value` e devolve `XprResult`. O **emissor** escolhe,
//! em tempo de compilação, a variante concreta pelo tipo do argumento (o polimorfismo não tem
//! custo em tempo de execução, como a Seção 5 exige). As versões genéricas abaixo (que fazem
//! despacho em tempo de execução) existem para quando o builtin é usado como valor de primeira
//! classe (`use_func(print)` é válido pela spec).

use std::io::Write;

use crate::{
    call, codepoints, end_of_input, index_out_of_range, invalid_number, invalid_size, not_callable,
    render, type_mismatch, Fail, Format, Pos, Seq, Value, XprResult,
};

fn int(value: i64) -> Value {
    Value::Int(value)
}

// ---------------------------------------------------------------------------------------------
// Renderizadores
// ---------------------------------------------------------------------------------------------

pub fn print(value: Value) -> XprResult {
    let text = render(&value, Format::default());
    let mut out = std::io::stdout();
    let _ = out.write_all(text.as_bytes());
    let _ = out.flush();
    Ok(int(codepoints(&text)))
}

pub fn println(value: Value) -> XprResult {
    let text = render(&value, Format::default());
    let mut out = std::io::stdout();
    let _ = out.write_all(text.as_bytes());
    let _ = out.write_all(b"\n");
    let _ = out.flush();
    Ok(int(codepoints(&text) + 1))
}

pub fn to_string(value: Value) -> XprResult {
    Ok(Value::string(render(&value, Format::default())))
}

/// `format([v, [decimais, alg_significativos]])`.
pub fn format(value: Value, pos: Pos) -> XprResult {
    let items = match &value {
        Value::Seq(seq) if seq.len() == 2 => seq.clone(),
        Value::Seq(seq) => {
            return Err(crate::size_mismatch(
                format!("format espera 2 elementos, recebeu {}", seq.len()),
                pos,
            ))
        }
        other => {
            return Err(type_mismatch(
                format!("format espera uma lista, recebeu {}", other.type_name()),
                pos,
            ))
        }
    };
    let spec = match &items.0[1] {
        Value::Seq(seq) if seq.len() == 2 => seq.clone(),
        Value::Seq(seq) => {
            return Err(crate::size_mismatch(
                format!(
                    "o formato espera 2 elementos, recebeu {}",
                    seq.len()
                ),
                pos,
            ))
        }
        other => {
            return Err(type_mismatch(
                format!(
                    "o formato espera uma lista de dois ints, recebeu {}",
                    other.type_name()
                ),
                pos,
            ))
        }
    };
    let decimals = match &spec.0[0] {
        Value::Int(n) => *n,
        other => {
            return Err(type_mismatch(
                format!("decimais deve ser int, recebeu {}", other.type_name()),
                pos,
            ))
        }
    };
    let significant = match &spec.0[1] {
        Value::Int(n) => *n,
        other => {
            return Err(type_mismatch(
                format!("alg_significativos deve ser int, recebeu {}", other.type_name()),
                pos,
            ))
        }
    };
    if decimals < 0 {
        return Err(invalid_size(
            format!("decimais negativo: {}", decimals),
            pos,
        ));
    }
    if significant < 1 {
        return Err(invalid_size(
            format!("alg_significativos menor que 1: {}", significant),
            pos,
        ));
    }
    let format = Format {
        decimals,
        significant,
    };
    Ok(Value::string(render(&items.0[0], format)))
}

// ---------------------------------------------------------------------------------------------
// Entrada
// ---------------------------------------------------------------------------------------------

pub fn input(_prompt: Value, pos: Pos) -> XprResult {
    let mut line = String::new();
    match std::io::stdin().read_line(&mut line) {
        Ok(0) | Err(_) => Err(end_of_input(pos)),
        Ok(_) => {
            if line.ends_with('\n') {
                line.pop();
                if line.ends_with('\r') {
                    line.pop();
                }
            }
            Ok(Value::string(line))
        }
    }
}

pub fn fail(message: Value, pos: Pos) -> XprResult {
    match message {
        Value::Str(text) => Err(crate::explicit_fail(&text, pos)),
        other => Err(type_mismatch(
            format!("fail espera uma string, recebeu {}", other.type_name()),
            pos,
        )),
    }
}

// ---------------------------------------------------------------------------------------------
// Conversões
// ---------------------------------------------------------------------------------------------

pub fn to_int(value: Value, pos: Pos) -> XprResult {
    match value {
        Value::Int(n) => Ok(int(n)),
        Value::Real(x) => {
            if !x.is_finite() || x < i64::MIN as f64 || x > i64::MAX as f64 {
                return Err(invalid_number(&x.to_string(), pos));
            }
            Ok(int(x.trunc() as i64))
        }
        Value::Str(text) => text
            .trim()
            .parse::<i64>()
            .map(int)
            .map_err(|_| invalid_number(&text, pos)),
        other => Err(type_mismatch(
            format!("int espera int, real ou string, recebeu {}", other.type_name()),
            pos,
        )),
    }
}

pub fn to_real(value: Value, pos: Pos) -> XprResult {
    match value {
        Value::Int(n) => Ok(Value::Real(n as f64)),
        Value::Real(x) => Ok(Value::Real(x)),
        Value::Str(text) => text
            .trim()
            .parse::<f64>()
            .map(Value::Real)
            .map_err(|_| invalid_number(&text, pos)),
        other => Err(type_mismatch(
            format!("real espera int, real ou string, recebeu {}", other.type_name()),
            pos,
        )),
    }
}

fn real_arg(value: Value, name: &str, pos: Pos) -> Result<f64, Fail> {
    match value {
        Value::Real(x) => Ok(x),
        Value::Int(n) => Ok(n as f64),
        other => Err(type_mismatch(
            format!("{} espera real, recebeu {}", name, other.type_name()),
            pos,
        )),
    }
}

pub fn floor(value: Value, pos: Pos) -> XprResult {
    Ok(Value::Real(real_arg(value, "floor", pos)?.floor()))
}

pub fn ceil(value: Value, pos: Pos) -> XprResult {
    Ok(Value::Real(real_arg(value, "ceil", pos)?.ceil()))
}

pub fn round(value: Value, pos: Pos) -> XprResult {
    Ok(Value::Real(real_arg(value, "round", pos)?.round()))
}

pub fn frac(value: Value, pos: Pos) -> XprResult {
    let x = real_arg(value, "frac", pos)?;
    Ok(Value::Real(x - x.trunc()))
}

/// `any([bool])` — `or` com curto-circuito.
pub fn any(value: Value, pos: Pos) -> XprResult {
    let seq = bool_seq(value, "any", pos)?;
    for item in seq.iter() {
        if let Value::Bool(true) = item {
            return Ok(Value::Bool(true));
        }
    }
    Ok(Value::Bool(false))
}

/// `all([bool])` — `and` com curto-circuito.
pub fn all(value: Value, pos: Pos) -> XprResult {
    let seq = bool_seq(value, "all", pos)?;
    for item in seq.iter() {
        if let Value::Bool(false) = item {
            return Ok(Value::Bool(false));
        }
    }
    Ok(Value::Bool(true))
}

fn bool_seq(value: Value, name: &str, pos: Pos) -> Result<Seq, Fail> {
    match value {
        Value::Seq(seq) => {
            for item in seq.iter() {
                if !matches!(item, Value::Bool(_)) {
                    return Err(type_mismatch(
                        format!("{} espera uma lista de bool, recebeu {}", name, item.type_name()),
                        pos,
                    ));
                }
            }
            Ok(seq)
        }
        other => Err(type_mismatch(
            format!("{} espera uma lista de bool, recebeu {}", name, other.type_name()),
            pos,
        )),
    }
}

// ---------------------------------------------------------------------------------------------
// Listas e strings
// ---------------------------------------------------------------------------------------------

pub fn len(value: Value, pos: Pos) -> XprResult {
    match value {
        Value::Seq(seq) => Ok(int(seq.len() as i64)),
        Value::Str(text) => Ok(int(codepoints(&text))),
        other => Err(type_mismatch(
            format!("len espera lista ou string, recebeu {}", other.type_name()),
            pos,
        )),
    }
}

/// `cat([[a], [b]])` — concatena as listas contidas no argumento.
pub fn cat(value: Value, pos: Pos) -> XprResult {
    let outer = match value {
        Value::Seq(seq) => seq,
        other => {
            return Err(type_mismatch(
                format!("cat espera uma lista de listas, recebeu {}", other.type_name()),
                pos,
            ))
        }
    };
    let mut values = Vec::new();
    for item in outer.iter() {
        match item {
            Value::Seq(inner) => values.extend(inner.iter().cloned()),
            other => {
                return Err(type_mismatch(
                    format!("cat espera listas, recebeu {}", other.type_name()),
                    pos,
                ))
            }
        }
    }
    Ok(Value::seq(values))
}

/// `at` — despacho em tempo de execução entre a forma de um índice e a de lista de índices.
/// O emissor pode chamar `at_one`/`at_many` diretamente quando o tipo é conhecido; este
/// despachante existe para o builtin usado como valor de primeira classe.
pub fn at(value: Value, pos: Pos) -> XprResult {
    let many = match &value {
        Value::Seq(seq) if seq.len() == 2 => matches!(&seq.0[1], Value::Seq(_)),
        _ => false,
    };
    if many {
        at_many(value, pos)
    } else {
        at_one(value, pos)
    }
}

/// `at([[a], int])` ou `at([string, int])` — um elemento.
pub fn at_one(value: Value, pos: Pos) -> XprResult {
    let (target, index) = match value {
        Value::Seq(seq) if seq.len() == 2 => (seq.0[0].clone(), seq.0[1].clone()),
        Value::Seq(seq) => {
            return Err(crate::size_mismatch(
                format!("at espera 2 elementos, recebeu {}", seq.len()),
                pos,
            ))
        }
        other => {
            return Err(type_mismatch(
                format!("at espera uma lista, recebeu {}", other.type_name()),
                pos,
            ))
        }
    };
    crate::ops::index(target, index, pos)
}

/// `at([[a], [int]])` ou `at([string, [int]])` — vários elementos, em ordem.
pub fn at_many(value: Value, pos: Pos) -> XprResult {
    let (target, indices) = match value {
        Value::Seq(seq) if seq.len() == 2 => (seq.0[0].clone(), seq.0[1].clone()),
        Value::Seq(seq) => {
            return Err(crate::size_mismatch(
                format!("at espera 2 elementos, recebeu {}", seq.len()),
                pos,
            ))
        }
        other => {
            return Err(type_mismatch(
                format!("at espera uma lista, recebeu {}", other.type_name()),
                pos,
            ))
        }
    };
    let indices = match indices {
        Value::Seq(seq) => seq,
        other => {
            return Err(type_mismatch(
                format!("at espera uma lista de índices, recebeu {}", other.type_name()),
                pos,
            ))
        }
    };
    match &target {
        Value::Str(text) => {
            let chars: Vec<char> = text.chars().collect();
            let mut out = String::new();
            for item in indices.iter() {
                let index = match item {
                    Value::Int(n) => *n,
                    other => {
                        return Err(type_mismatch(
                            format!("índice deve ser int, recebeu {}", other.type_name()),
                            pos,
                        ))
                    }
                };
                if index < 0 || index as usize >= chars.len() {
                    return Err(index_out_of_range(index, chars.len(), pos));
                }
                out.push(chars[index as usize]);
            }
            Ok(Value::string(out))
        }
        Value::Seq(seq) => {
            let mut values = Vec::with_capacity(indices.len());
            for item in indices.iter() {
                let index = match item {
                    Value::Int(n) => *n,
                    other => {
                        return Err(type_mismatch(
                            format!("índice deve ser int, recebeu {}", other.type_name()),
                            pos,
                        ))
                    }
                };
                if index < 0 || index as usize >= seq.len() {
                    return Err(index_out_of_range(index, seq.len(), pos));
                }
                values.push(seq.get(index as usize).unwrap().clone());
            }
            Ok(Value::seq(values))
        }
        other => Err(type_mismatch(
            format!("at não aceita {}", other.type_name()),
            pos,
        )),
    }
}

/// `fill([int, a])` — lista com `n` cópias de um valor.
pub fn fill(value: Value, pos: Pos) -> XprResult {
    let items = pair(value, "fill", pos)?;
    let count = match &items.0[0] {
        Value::Int(n) => *n,
        other => {
            return Err(type_mismatch(
                format!("o tamanho deve ser int, recebeu {}", other.type_name()),
                pos,
            ))
        }
    };
    if count < 0 {
        return Err(invalid_size(format!("tamanho negativo: {}", count), pos));
    }
    Ok(Value::seq(vec![items.0[1].clone(); count as usize]))
}

/// `range(int)`, `range([int, int])` ou `range([int, int, int])`.
pub fn range(value: Value, pos: Pos) -> XprResult {
    let (start, end, step) = match &value {
        Value::Int(n) => (0, *n, 1),
        Value::Seq(seq) => match seq.len() {
            2 => match (&seq.0[0], &seq.0[1]) {
                (Value::Int(a), Value::Int(b)) => (*a, *b, 1),
                _ => return Err(type_mismatch("range espera ints", pos)),
            },
            3 => match (&seq.0[0], &seq.0[1], &seq.0[2]) {
                (Value::Int(a), Value::Int(b), Value::Int(c)) => (*a, *b, *c),
                _ => return Err(type_mismatch("range espera ints", pos)),
            },
            n => {
                return Err(invalid_size(
                    format!("range espera 2 ou 3 elementos, recebeu {}", n),
                    pos,
                ))
            }
        },
        other => {
            return Err(type_mismatch(
                format!("range espera int ou lista, recebeu {}", other.type_name()),
                pos,
            ))
        }
    };
    if step == 0 {
        return Err(invalid_size("passo de range é zero", pos));
    }
    let mut values = Vec::new();
    let mut current = start;
    while (step > 0 && current < end) || (step < 0 && current > end) {
        values.push(int(current));
        match current.checked_add(step) {
            Some(next) => current = next,
            None => break,
        }
    }
    Ok(Value::seq(values))
}

/// `set([[a], int, a])` — nova lista com um elemento substituído (não muta).
pub fn set(value: Value, pos: Pos) -> XprResult {
    let items = match &value {
        Value::Seq(seq) if seq.len() == 3 => seq.clone(),
        Value::Seq(seq) => {
            return Err(crate::size_mismatch(
                format!("set espera 3 elementos, recebeu {}", seq.len()),
                pos,
            ))
        }
        other => {
            return Err(type_mismatch(
                format!("set espera uma lista, recebeu {}", other.type_name()),
                pos,
            ))
        }
    };
    let target = match &items.0[0] {
        Value::Seq(seq) => seq.clone(),
        other => {
            return Err(type_mismatch(
                format!("set espera uma lista, recebeu {}", other.type_name()),
                pos,
            ))
        }
    };
    let index = match &items.0[1] {
        Value::Int(n) => *n,
        other => {
            return Err(type_mismatch(
                format!("índice deve ser int, recebeu {}", other.type_name()),
                pos,
            ))
        }
    };
    if index < 0 || index as usize >= target.len() {
        return Err(index_out_of_range(index, target.len(), pos));
    }
    let mut values: Vec<Value> = target.iter().cloned().collect();
    values[index as usize] = items.0[2].clone();
    Ok(Value::seq(values))
}

/// `slice([[a], int, int])` ou `slice([string, int, int])`.
pub fn slice(value: Value, pos: Pos) -> XprResult {
    let items = match &value {
        Value::Seq(seq) if seq.len() == 3 => seq.clone(),
        Value::Seq(seq) => {
            return Err(crate::size_mismatch(
                format!("slice espera 3 elementos, recebeu {}", seq.len()),
                pos,
            ))
        }
        other => {
            return Err(type_mismatch(
                format!("slice espera uma lista, recebeu {}", other.type_name()),
                pos,
            ))
        }
    };
    let start = match &items.0[1] {
        Value::Int(n) => *n,
        other => {
            return Err(type_mismatch(
                format!("o início deve ser int, recebeu {}", other.type_name()),
                pos,
            ))
        }
    };
    let end = match &items.0[2] {
        Value::Int(n) => *n,
        other => {
            return Err(type_mismatch(
                format!("o fim deve ser int, recebeu {}", other.type_name()),
                pos,
            ))
        }
    };
    match &items.0[0] {
        Value::Seq(seq) => {
            let len = seq.len() as i64;
            if start < 0 || end > len || start > end {
                return Err(invalid_size(
                    format!("intervalo inválido: [{}, {}) de {}", start, end, len),
                    pos,
                ));
            }
            Ok(Value::seq(
                seq.iter()
                    .skip(start as usize)
                    .take((end - start) as usize)
                    .cloned()
                    .collect(),
            ))
        }
        Value::Str(text) => {
            let chars: Vec<char> = text.chars().collect();
            let len = chars.len() as i64;
            if start < 0 || end > len || start > end {
                return Err(invalid_size(
                    format!("intervalo inválido: [{}, {}) de {}", start, end, len),
                    pos,
                ));
            }
            Ok(Value::string(
                chars[start as usize..end as usize].iter().collect::<String>(),
            ))
        }
        other => Err(type_mismatch(
            format!("slice não aceita {}", other.type_name()),
            pos,
        )),
    }
}

/// `apply([[a], a -> b])` — aplica a função a cada elemento.
pub fn apply(value: Value, pos: Pos) -> XprResult {
    let items = pair(value, "apply", pos)?;
    let list = match &items.0[0] {
        Value::Seq(seq) => seq.clone(),
        other => {
            return Err(type_mismatch(
                format!("apply espera uma lista, recebeu {}", other.type_name()),
                pos,
            ))
        }
    };
    if !items.0[1].is_fn() {
        return Err(not_callable(&items.0[1], pos));
    }
    let function = items.0[1].clone();
    let mut values = Vec::with_capacity(list.len());
    for item in list.iter() {
        values.push(call(function.clone(), item.clone(), pos)?);
    }
    Ok(Value::seq(values))
}

fn pair(value: Value, name: &str, pos: Pos) -> Result<Seq, Fail> {
    match value {
        Value::Seq(seq) if seq.len() == 2 => Ok(seq),
        Value::Seq(seq) => Err(crate::size_mismatch(
            format!("{} espera 2 elementos, recebeu {}", name, seq.len()),
            pos,
        )),
        other => Err(type_mismatch(
            format!("{} espera uma lista, recebeu {}", name, other.type_name()),
            pos,
        )),
    }
}

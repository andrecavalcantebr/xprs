//! Operadores da Seção 5.2 da especificação.
//!
//! Regras implementadas:
//!
//! * `int ⊗ int` → aritmética inteira (`/` é divisão inteira), com `div-by-zero` e `overflow`;
//! * `real ⊗ real` → aritmética de ponto flutuante;
//! * `int ⊗ real` (qualquer ordem) → promoção do int para real;
//! * `string ⊗ string` → `+` concatena; os demais são `type-mismatch`;
//! * lista/tupla ⊗ lista/tupla → elemento a elemento (posição a posição), com `size-mismatch` em
//!   tempo de execução (listas) ou em tempo de compilação (tuplas, checado no sema);
//! * escalar ⊗ lista → `type-mismatch` (sem broadcasting);
//! * bool ⊗ bool só em `==`/`!=`.

use crate::{
    div_by_zero, overflow, size_mismatch, type_mismatch, Pos, Value,
};

// ---------------------------------------------------------------------------------------------
// Aritmética
// ---------------------------------------------------------------------------------------------

#[inline]
pub fn add(lhs: Value, rhs: Value, pos: Pos) -> crate::XprResult {
    numeric(
        lhs,
        rhs,
        pos,
        &|a, b| a.checked_add(b),
        &|a, b| a + b,
        &|a, b| {
            let mut out = String::with_capacity(a.len() + b.len());
            out.push_str(a);
            out.push_str(b);
            Some(out)
        },
    )
}

#[inline]
pub fn sub(lhs: Value, rhs: Value, pos: Pos) -> crate::XprResult {
    numeric(
        lhs,
        rhs,
        pos,
        &|a, b| a.checked_sub(b),
        &|a, b| a - b,
        &|_, _| None,
    )
}

#[inline]
pub fn mul(lhs: Value, rhs: Value, pos: Pos) -> crate::XprResult {
    numeric(
        lhs,
        rhs,
        pos,
        &|a, b| a.checked_mul(b),
        &|a, b| a * b,
        &|_, _| None,
    )
}

#[inline]
pub fn div(lhs: Value, rhs: Value, pos: Pos) -> crate::XprResult {
    match (&lhs, &rhs) {
        (Value::Int(_), Value::Int(0)) => Err(div_by_zero(pos)),
        (Value::Real(_), Value::Real(b)) if *b == 0.0 => Err(div_by_zero(pos)),
        _ => numeric(
            lhs,
            rhs,
            pos,
            &|a, b| if b == 0 { None } else { a.checked_div(b) },
            &|a, b| a / b,
            &|_, _| None,
        ),
    }
}

#[inline]
pub fn rem(lhs: Value, rhs: Value, pos: Pos) -> crate::XprResult {
    match (&lhs, &rhs) {
        (Value::Int(_), Value::Int(0)) => Err(div_by_zero(pos)),
        (Value::Real(_), Value::Real(b)) if *b == 0.0 => Err(div_by_zero(pos)),
        _ => numeric(
            lhs,
            rhs,
            pos,
            &|a, b| if b == 0 { None } else { a.checked_rem(b) },
            &|a, b| a % b,
            &|_, _| None,
        ),
    }
}

/// Núcleo comum de `+ - * / %`: tenta inteiro, depois real, depois string (só `+`), e por fim
/// elemento a elemento para sequências.
fn numeric(
    lhs: Value,
    rhs: Value,
    pos: Pos,
    int_op: &dyn Fn(i64, i64) -> Option<i64>,
    real_op: &dyn Fn(f64, f64) -> f64,
    str_op: &dyn Fn(&str, &str) -> Option<String>,
) -> crate::XprResult {
    match (&lhs, &rhs) {
        (Value::Int(a), Value::Int(b)) => match int_op(*a, *b) {
            Some(value) => Ok(Value::Int(value)),
            None => Err(overflow(pos)),
        },
        (Value::Int(a), Value::Real(b)) => Ok(Value::Real(real_op(*a as f64, *b))),
        (Value::Real(a), Value::Int(b)) => Ok(Value::Real(real_op(*a, *b as f64))),
        (Value::Real(a), Value::Real(b)) => Ok(Value::Real(real_op(*a, *b))),
        (Value::Str(a), Value::Str(b)) => match str_op(a, b) {
            Some(text) => Ok(Value::string(text)),
            None => Err(type_mismatch(
                format!("operação indefinida entre string e string"),
                pos,
            )),
        },
        (Value::Seq(a), Value::Seq(b)) => {
            if a.len() != b.len() {
                return Err(size_mismatch(
                    format!("comprimentos {} e {}", a.len(), b.len()),
                    pos,
                ));
            }
            let mut values = Vec::with_capacity(a.len());
            for (left, right) in a.iter().zip(b.iter()) {
                values.push(numeric(left.clone(), right.clone(), pos, int_op, real_op, str_op)?);
            }
            Ok(Value::seq(values))
        }
        _ => Err(type_mismatch(
            format!("tipos {} e {}", lhs.type_name(), rhs.type_name()),
            pos,
        )),
    }
}

// ---------------------------------------------------------------------------------------------
// Comparações
// ---------------------------------------------------------------------------------------------

#[inline]
pub fn lt(lhs: Value, rhs: Value, pos: Pos) -> crate::XprResult {
    compare(lhs, rhs, pos, |ord| ord == std::cmp::Ordering::Less)
}

#[inline]
pub fn le(lhs: Value, rhs: Value, pos: Pos) -> crate::XprResult {
    compare(lhs, rhs, pos, |ord| ord != std::cmp::Ordering::Greater)
}

#[inline]
pub fn gt(lhs: Value, rhs: Value, pos: Pos) -> crate::XprResult {
    compare(lhs, rhs, pos, |ord| ord == std::cmp::Ordering::Greater)
}

#[inline]
pub fn ge(lhs: Value, rhs: Value, pos: Pos) -> crate::XprResult {
    compare(lhs, rhs, pos, |ord| ord != std::cmp::Ordering::Less)
}

/// `NaN` nunca é menor/maior/igual a nada (IEEE-754, Seção 5.2).
fn compare(
    lhs: Value,
    rhs: Value,
    pos: Pos,
    accept: fn(std::cmp::Ordering) -> bool,
) -> crate::XprResult {
    match (&lhs, &rhs) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Bool(accept(a.cmp(b)))),
        (Value::Int(a), Value::Real(b)) => Ok(Value::Bool(real_lt(*a as f64, *b, accept))),
        (Value::Real(a), Value::Int(b)) => Ok(Value::Bool(real_lt(*a, *b as f64, accept))),
        (Value::Real(a), Value::Real(b)) => {
            Ok(Value::Bool(match a.partial_cmp(b) {
                Some(ord) => accept(ord),
                None => false, // NaN
            }))
        }
        (Value::Str(a), Value::Str(b)) => Ok(Value::Bool(accept(a.cmp(b)))),
        (Value::Seq(a), Value::Seq(b)) => {
            if a.len() != b.len() {
                return Err(size_mismatch(
                    format!("comprimentos {} e {}", a.len(), b.len()),
                    pos,
                ));
            }
            let mut values = Vec::with_capacity(a.len());
            for (left, right) in a.iter().zip(b.iter()) {
                values.push(compare(left.clone(), right.clone(), pos, accept)?);
            }
            Ok(Value::seq(values))
        }
        _ => Err(type_mismatch(
            format!("comparação entre {} e {}", lhs.type_name(), rhs.type_name()),
            pos,
        )),
    }
}

fn real_lt(a: f64, b: f64, accept: fn(std::cmp::Ordering) -> bool) -> bool {
    match a.partial_cmp(&b) {
        Some(ord) => accept(ord),
        None => false,
    }
}

/// Igualdade estrutural (`==`), com promoção int→real.
#[inline]
pub fn eq(lhs: Value, rhs: Value, pos: Pos) -> crate::XprResult {
    equal(lhs, rhs, pos, true)
}

#[inline]
pub fn ne(lhs: Value, rhs: Value, pos: Pos) -> crate::XprResult {
    equal(lhs, rhs, pos, false)
}

fn equal(lhs: Value, rhs: Value, pos: Pos, want_equal: bool) -> crate::XprResult {
    match (&lhs, &rhs) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Bool((a == b) == want_equal)),
        (Value::Int(a), Value::Real(b)) => Ok(Value::Bool(((*a as f64) == *b) == want_equal)),
        (Value::Real(a), Value::Int(b)) => Ok(Value::Bool((*a == (*b as f64)) == want_equal)),
        (Value::Real(a), Value::Real(b)) => Ok(Value::Bool((a == b) == want_equal)),
        (Value::Str(a), Value::Str(b)) => Ok(Value::Bool((a == b) == want_equal)),
        (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool((a == b) == want_equal)),
        (Value::Seq(a), Value::Seq(b)) => {
            if a.len() != b.len() {
                return Err(size_mismatch(
                    format!("comprimentos {} e {}", a.len(), b.len()),
                    pos,
                ));
            }
            let mut values = Vec::with_capacity(a.len());
            for (left, right) in a.iter().zip(b.iter()) {
                values.push(equal(left.clone(), right.clone(), pos, want_equal)?);
            }
            Ok(Value::seq(values))
        }
        _ => Err(type_mismatch(
            format!("comparação entre {} e {}", lhs.type_name(), rhs.type_name()),
            pos,
        )),
    }
}

// ---------------------------------------------------------------------------------------------
// Negação unária
// ---------------------------------------------------------------------------------------------

/// `not` — apenas bool.
#[inline]
pub fn not(value: Value, pos: Pos) -> crate::XprResult {
    match value {
        Value::Bool(b) => Ok(Value::Bool(!b)),
        other => Err(type_mismatch(
            format!("not exige bool, recebeu {}", other.type_name()),
            pos,
        )),
    }
}

/// Sinal unário `-`: números, com promoção e elemento a elemento em listas/tuplas numéricas.
#[inline]
pub fn neg(value: Value, pos: Pos) -> crate::XprResult {
    match value {
        Value::Int(n) => n
            .checked_neg()
            .map(Value::Int)
            .ok_or_else(|| overflow(pos)),
        Value::Real(x) => Ok(Value::Real(-x)),
        Value::Seq(seq) => {
            let mut values = Vec::with_capacity(seq.len());
            for item in seq.iter() {
                values.push(neg(item.clone(), pos)?);
            }
            Ok(Value::seq(values))
        }
        other => Err(type_mismatch(
            format!(
                "o sinal unário exige um valor numérico, recebeu {}",
                other.type_name()
            ),
            pos,
        )),
    }
}

/// Sinal unário `+`: valida que é numérico, sem alterar o valor.
#[inline]
pub fn plus(value: Value, pos: Pos) -> crate::XprResult {
    match value {
        Value::Int(_) | Value::Real(_) => Ok(value),
        Value::Seq(seq) => {
            let mut values = Vec::with_capacity(seq.len());
            for item in seq.iter() {
                values.push(plus(item.clone(), pos)?);
            }
            Ok(Value::seq(values))
        }
        other => Err(type_mismatch(
            format!(
                "o sinal unário exige um valor numérico, recebeu {}",
                other.type_name()
            ),
            pos,
        )),
    }
}

// ---------------------------------------------------------------------------------------------
// Indexação (Seção 4.8)
// ---------------------------------------------------------------------------------------------

/// `valor[indice]` com índice int. Strings retornam uma string de um codepoint.
#[inline]
pub fn index(value: Value, index_value: Value, pos: Pos) -> crate::XprResult {
    let index = match index_value {
        Value::Int(n) => n,
        other => {
            return Err(type_mismatch(
                format!("índice deve ser int, recebeu {}", other.type_name()),
                pos,
            ))
        }
    };
    match value {
        Value::Seq(seq) => {
            if index < 0 || index as usize >= seq.len() {
                return Err(crate::index_out_of_range(index, seq.len(), pos));
            }
            Ok(seq.get(index as usize).unwrap().clone())
        }
        Value::Str(text) => {
            let chars: Vec<char> = text.chars().collect();
            if index < 0 || index as usize >= chars.len() {
                return Err(crate::index_out_of_range(index, chars.len(), pos));
            }
            Ok(Value::string(chars[index as usize].to_string()))
        }
        other => Err(type_mismatch(
            format!("{} não pode ser indexado", other.type_name()),
            pos,
        )),
    }
}

// ---------------------------------------------------------------------------------------------
// Testes unitários das regras da Seção 5.2
// ---------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{render, Format};

    fn int(n: i64) -> Value {
        Value::Int(n)
    }

    fn as_int(value: Value) -> i64 {
        match value {
            Value::Int(n) => n,
            other => panic!("esperava int, veio {:?}", other),
        }
    }

    fn as_real(value: Value) -> f64 {
        match value {
            Value::Real(n) => n,
            other => panic!("esperava real, veio {:?}", other),
        }
    }

    fn as_text(value: Value) -> String {
        render(&value, Format::default())
    }

    #[test]
    fn soma_de_inteiros_com_overflow() {
        assert_eq!(as_int(add(int(1), int(2), None).unwrap()), 3);
        assert_eq!(
            add(int(i64::MAX), int(1), None).unwrap_err().diag.code,
            crate::DiagCode::Overflow
        );
    }

    #[test]
    fn divisao_inteira_e_por_zero() {
        assert_eq!(as_int(div(int(1), int(2), None).unwrap()), 0);
        assert_eq!(
            div(int(1), int(0), None).unwrap_err().diag.code,
            crate::DiagCode::DivByZero
        );
    }

    #[test]
    fn promocao_int_para_real_nas_duas_ordens() {
        assert_eq!(as_real(add(int(1), Value::Real(2.5), None).unwrap()), 3.5);
        assert_eq!(as_real(add(Value::Real(2.5), int(1), None).unwrap()), 3.5);
        assert_eq!(as_real(div(Value::Real(1.0), Value::Real(2.0), None).unwrap()), 0.5);
    }

    #[test]
    fn resto_de_negativo_segue_o_dividendo() {
        assert_eq!(as_int(rem(int(-7), int(3), None).unwrap()), -1);
    }

    #[test]
    fn concatenacao_de_strings_e_rejeicao_dos_demais() {
        assert_eq!(
            as_text(add(Value::string("a"), Value::string("b"), None).unwrap()),
            "ab"
        );
        assert_eq!(
            sub(Value::string("a"), Value::string("b"), None).unwrap_err().diag.code,
            crate::DiagCode::TypeMismatch
        );
    }

    #[test]
    fn aritmetica_elemento_a_elemento_em_listas() {
        let a = Value::seq(vec![int(1), int(2)]);
        let b = Value::seq(vec![int(10), int(20)]);
        assert_eq!(as_text(add(a, b, None).unwrap()), "[11, 22]");
    }

    #[test]
    fn tamanhos_diferentes_em_tempo_de_execucao() {
        let a = Value::seq(vec![int(1), int(2)]);
        let b = Value::seq(vec![int(10)]);
        assert_eq!(
            add(a, b, None).unwrap_err().diag.code,
            crate::DiagCode::SizeMismatch
        );
    }

    #[test]
    fn escalar_com_lista_e_type_mismatch() {
        let a = Value::seq(vec![int(1)]);
        assert_eq!(
            add(a, int(1), None).unwrap_err().diag.code,
            crate::DiagCode::TypeMismatch
        );
    }

    #[test]
    fn comparacao_de_strings_e_lexicografica_por_codepoint() {
        let a = Value::string("A");
        let b = Value::string("a");
        match lt(a, b, None).unwrap() {
            Value::Bool(b) => assert!(b),
            other => panic!("esperava bool, veio {:?}", other),
        }
    }

    #[test]
    fn nan_nunca_compara() {
        let nan = Value::Real(f64::NAN);
        for outcome in [
            lt(nan.clone(), nan.clone(), None).unwrap(),
            eq(nan.clone(), nan.clone(), None).unwrap(),
        ] {
            match outcome {
                Value::Bool(b) => assert!(!b),
                other => panic!("esperava bool, veio {:?}", other),
            }
        }
    }

    #[test]
    fn negacao_de_seq_e_elemento_a_elemento() {
        let values = Value::seq(vec![int(10), int(-11), int(12)]);
        assert_eq!(as_text(neg(values, None).unwrap()), "[-10, 11, -12]");
    }

    #[test]
    fn negacao_de_string_e_type_mismatch() {
        assert_eq!(
            neg(Value::string("x"), None).unwrap_err().diag.code,
            crate::DiagCode::TypeMismatch
        );
    }

    #[test]
    fn indexacao_de_lista_e_string() {
        let list = Value::seq(vec![int(10), int(20)]);
        assert_eq!(as_int(index(list.clone(), int(1), None).unwrap()), 20);
        assert_eq!(
            index(list, int(2), None).unwrap_err().diag.code,
            crate::DiagCode::IndexOutOfRange
        );
        assert_eq!(
            as_text(index(Value::string("ação"), int(1), None).unwrap()),
            "ç"
        );
    }
}

#!/usr/bin/env bash
# Roda a etapa Lexer completa e dá o veredito OK/NOK:
#   1. testes unitários por subetapa (tests/lexer_tests.rs, subetapas 1-7 de plano-lexer.md);
#   2. suíte golden-file (tests/lexer_fixtures.rs sobre tests/lexer/cases/).
# Única dependência: o toolchain Rust (cargo/rustc) que o projeto já exige para tudo o mais.
#
# O teste de fixtures imprime "[OK] <nome>" ou "[FAIL] <nome>: <motivo>" por caso (visível com
# --nocapture) e entra em panic com um resumo se qualquer fixture falhar — é isso que faz
# `cargo test` sair com status != 0, que é o que este script propaga.
set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

status=0

echo "== Lexer: testes unitários por subetapa (tests/lexer_tests.rs) =="
if cargo test --test lexer_tests; then
  echo "OK: testes unitários do lexer passaram."
else
  echo "NOK: testes unitários do lexer falharam."
  status=1
fi

echo
echo "== Lexer: fixtures golden-file (tests/lexer/cases/) =="
if cargo test --test lexer_fixtures -- --nocapture; then
  echo "OK: todas as fixtures do lexer passaram."
else
  echo "NOK: pelo menos uma fixture do lexer falhou (ver [FAIL] acima)."
  status=1
fi

if [ "$status" -eq 0 ]; then
  echo
  echo "OK: etapa Lexer completa (unitários + fixtures)."
fi
exit "$status"

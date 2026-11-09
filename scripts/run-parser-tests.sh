#!/usr/bin/env bash
# Roda a etapa Parser completa e dá o veredito OK/NOK:
#   1. testes unitários por subetapa (tests/parser_tests.rs, subetapas A-J de plano-parser.md);
#   2. suíte golden-file (tests/parser_fixtures.rs sobre tests/parser/cases/).
# Única dependência: o toolchain Rust (cargo/rustc) que o projeto já exige para tudo o mais.
#
# O teste de fixtures imprime "[OK] <nome>" ou "[FAIL] <nome>: <motivo>" por caso (visível com
# --nocapture) e entra em panic com um resumo se qualquer fixture falhar — é isso que faz
# `cargo test` sair com status != 0, que é o que este script propaga.
set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

status=0

echo "== Parser: testes unitários por subetapa (tests/parser_tests.rs) =="
if cargo test --test parser_tests; then
  echo "OK: testes unitários do parser passaram."
else
  echo "NOK: testes unitários do parser falharam."
  status=1
fi

echo
echo "== Parser: fixtures golden-file (tests/parser/cases/) =="
if cargo test --test parser_fixtures -- --nocapture; then
  echo "OK: todas as fixtures do parser passaram."
else
  echo "NOK: pelo menos uma fixture do parser falhou (ver [FAIL] acima)."
  status=1
fi

if [ "$status" -eq 0 ]; then
  echo
  echo "OK: etapa Parser completa (unitários + fixtures)."
fi
exit "$status"

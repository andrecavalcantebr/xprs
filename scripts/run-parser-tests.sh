#!/usr/bin/env bash
# Roda a suíte de fixtures golden-file do Parser (tests/parser/cases/) e dá o veredito OK/NOK.
# Mesma ideia de scripts/run-lexer-tests.sh — ver esse arquivo para o racional completo.
#
# Pressupõe um teste de integração `tests/parser_fixtures.rs` (ver plano-parser.md) que:
#   - itera tests/parser/cases/*.x (sucesso, par com .ast) e
#     tests/parser/cases/errors/*.x (erro, par com .error);
#   - compara o dump de AST (formato em plano-parser.md) ou o SyntaxErrorKind+span esperado;
#   - imprime "[OK] <nome>" / "[FAIL] <nome>: <motivo>" por fixture, e falha a suíte inteira
#     (panic) se qualquer uma não passar.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

echo "== Parser: fixtures golden-file (tests/parser/cases/) =="
if cargo test --test parser_fixtures -- --nocapture; then
  echo "OK: todas as fixtures do parser passaram."
  exit 0
else
  echo "NOK: pelo menos uma fixture do parser falhou (ver [FAIL] acima)."
  exit 1
fi

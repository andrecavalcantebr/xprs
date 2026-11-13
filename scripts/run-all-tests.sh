#!/usr/bin/env bash
# Roda lexer -> parser -> milestones de codegen, nessa ordem, parando no primeiro grupo que
# falhar (não compensa rodar codegen com o parser quebrado). Veredito final único no fim.
set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

status=0

./scripts/run-lexer-tests.sh || status=1

if [ "$status" -eq 0 ]; then
  ./scripts/run-parser-tests.sh || status=1
fi

if [ "$status" -eq 0 ]; then
  # Testes Rust da crate (unitários da lib + integração), que antes ficavam fora do gate.
  cargo test --quiet || status=1
fi

if [ "$status" -eq 0 ]; then
  ./scripts/run-codegen-milestones.sh || status=1
fi

echo "===================="
if [ "$status" -eq 0 ]; then
  echo "TUDO OK"
else
  echo "HÁ FALHAS — ver [FAIL]/NOK acima"
fi
exit "$status"

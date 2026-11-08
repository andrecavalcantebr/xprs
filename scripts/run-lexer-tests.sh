#!/usr/bin/env bash
# Roda a suíte de fixtures golden-file do Lexer (tests/lexer/cases/) e dá o veredito OK/NOK.
# Única dependência: o toolchain Rust (cargo/rustc) que o projeto já exige para tudo o mais —
# nenhuma dependência extra.
#
# Pressupõe um teste de integração `tests/lexer_fixtures.rs` (ver plano-lexer.md) que:
#   - itera tests/lexer/cases/*.x (sucesso, par com .tokens) e
#     tests/lexer/cases/errors/*.x (erro, par com .error);
#   - para cada um, tokeniza e compara o dump formatado (ver formato exato em plano-lexer.md)
#     com o arquivo-irmão esperado;
#   - imprime "[OK] <nome>" ou "[FAIL] <nome>: <motivo>" por fixture (visível com --nocapture);
#   - ao final, entra em panic com um resumo se qualquer fixture falhou — é isso que faz
#     `cargo test` sair com status != 0, que é o que este script propaga.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

echo "== Lexer: fixtures golden-file (tests/lexer/cases/) =="
if cargo test --test lexer_fixtures -- --nocapture; then
  echo "OK: todas as fixtures do lexer passaram."
  exit 0
else
  echo "NOK: pelo menos uma fixture do lexer falhou (ver [FAIL] acima)."
  exit 1
fi

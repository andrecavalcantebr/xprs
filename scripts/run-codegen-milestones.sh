#!/usr/bin/env bash
# Roda os milestones de ponta a ponta do Codegen (tests/codegen/milestones/MNN_nome/) contra o
# binário xprc real: compila cada milestone, executa o binário gerado, e compara
# stdout/exit/stderr com o esperado. Ver MEMORY.md para o formato de cada
# milestone e a regra de que a suíte só cresce (nunca remove um milestone antigo).
#
# Única dependência: o toolchain Rust do projeto (cargo/rustc) + utilitários POSIX padrão
# (mktemp, grep, diff). Nenhuma dependência extra.
set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

MILESTONES_DIR="tests/codegen/milestones"
WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT

echo "== Codegen: milestones de ponta a ponta ($MILESTONES_DIR) =="
echo "-- compilando xprc --"
if ! cargo build --bin xprc --quiet; then
  echo "NOK: xprc nao compila — nenhum milestone pode ser testado."
  exit 1
fi
XPRC="target/debug/xprc"

pass=0
fail=0

for dir in "$MILESTONES_DIR"/*/; do
  name="$(basename "$dir")"
  entry="${dir}main.x"
  if [ ! -f "$entry" ]; then
    # milestone de um único arquivo, sem convenção "main.x"
    entry="$(find "$dir" -maxdepth 1 -name '*.x' | sort | head -n1)"
  fi
  exe="$WORKDIR/$name.exe"

  if ! "$XPRC" "$entry" -o "$exe" 2>"$WORKDIR/$name.compile.stderr"; then
    echo "[FAIL] $name: xprc não compilou (ver $WORKDIR/$name.compile.stderr)"
    fail=$((fail + 1))
    continue
  fi

  out="$("$exe" 2>"$WORKDIR/$name.stderr")"
  code=$?

  ok=1
  reason=""

  if [ -f "${dir}expected.stdout" ]; then
    want_out="$(cat "${dir}expected.stdout")"
    if [ "$out" != "$want_out" ]; then
      ok=0
      reason="${reason} stdout difere (esperado em ${dir}expected.stdout);"
    fi
  fi

  if [ -f "${dir}expected.exit.nonzero" ]; then
    if [ "$code" -eq 0 ]; then
      ok=0
      reason="${reason} esperava exit != 0, obteve 0;"
    fi
  elif [ -f "${dir}expected.exit" ]; then
    want_exit="$(cat "${dir}expected.exit")"
    if [ "$code" != "$want_exit" ]; then
      ok=0
      reason="${reason} exit=$code esperado=$want_exit;"
    fi
  else
    if [ "$code" -ne 0 ]; then
      ok=0
      reason="${reason} exit=$code esperado=0 (padrão quando não há expected.exit*);"
    fi
  fi

  if [ -f "${dir}expected.stderr.contains" ]; then
    needle="$(cat "${dir}expected.stderr.contains")"
    if ! grep -qF "$needle" "$WORKDIR/$name.stderr"; then
      ok=0
      reason="${reason} stderr não contém '$needle';"
    fi
  fi

  if [ "$ok" -eq 1 ]; then
    echo "[OK] $name"
    pass=$((pass + 1))
  else
    echo "[FAIL] $name:$reason"
    fail=$((fail + 1))
  fi
done

echo "----"
echo "$pass passaram, $fail falharam."
[ "$fail" -eq 0 ]

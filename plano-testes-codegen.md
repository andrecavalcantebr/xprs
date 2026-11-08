# Plano de testes — Codegen (suíte evolutiva)

Diferente do Lexer e do Parser (gramática fechada → fixtures golden-file fixas, ver
`plano-lexer.md`/`plano-parser.md`), o Codegen não tem um oráculo interno fixo: o que se testa é
**comportamento observável de ponta a ponta** — compilar um `.x` com `xprc`, rodar o executável
gerado, e comparar `stdout`/status de saída com o esperado. Por isso este documento não é uma
lista fechada de fixtures; é um **roteiro vivo de milestones**, cada um introduzindo um recorte
novo de construções da linguagem, alinhado às subetapas A–G de `plano-codegen.md`.

**Regra de evolução** (operacionaliza "a suíte evolui ao longo do tempo"): ao concluir qualquer
subetapa que introduza comportamento observável novo, adicione pelo menos um milestone novo a
este arquivo (com os `.x` de exemplo + saída esperada) **antes** de considerar aquela subetapa
concluída. Milestones já registrados nunca são removidos nem relaxados — a suíte só cresce.
Nenhum milestone pode ser alterado para acomodar um bug do compilador (AGENTS.md, 3.3.1): se a
saída esperada abaixo estiver errada em relação à especificação, é um bug deste documento,
reportado e corrigido abertamente.

## Gate

Uma subetapa (A–G) só é considerada concluída quando **todos** os milestones já registrados que a
exercitam (coluna "Subetapas" abaixo) — e todos os milestones de subetapas anteriores —
continuam passando, via `scripts/run-codegen-milestones.sh`. Regressão em um milestone antigo
bloqueia a conclusão de qualquer subetapa nova até ser corrigida.

## Formato de cada milestone

Diretório `tests/codegen/milestones/MNN_nome/`:
* um ou mais arquivos `.x` (o módulo de entrada sempre se chama `main.x` quando houver mais de
  um arquivo);
* `expected.stdout` — stdout exato esperado do executável (se omitido, não é verificado);
* `expected.exit` — código de saída esperado, um inteiro decimal (padrão implícito `0` se o
  arquivo não existir);
* `expected.exit.nonzero` — arquivo-marcador vazio; quando presente, substitui `expected.exit` e
  exige apenas status **≠ 0** (usado quando a spec não fixa um código exato, como uma falha não
  capturada);
* `expected.stderr.contains` — quando presente, seu conteúdo deve aparecer como substring do
  stderr do executável (verificação parcial, não igualdade exata).

## Milestones

### M0 — fatia mínima de ponta a ponta

Antes de M0 passar, nada de E/F/G funciona ainda de ponta a ponta — este é o "hello world" que
prova que lex→parse→sema→emissão→`rustc`→execução já formam um pipeline fechado.

```
_main = fn(args){
  println("hello, xpr");
  0;
};
```
`expected.stdout`: `hello, xpr\n`. `expected.exit`: `0`.

**Subetapas exercitadas**: E, F, G (fiação mínima), e de passagem A/B/C (nomes, closures triviais,
tipos escalares).

### M1 — aritmética e promoção int/real

```
_main = fn(args){
  println(string(1 + 2 * 3));
  println(string(1 / 2));
  println(string(real(1) / real(2)));
  println(string(-7 % 3));
  0;
};
```
`expected.stdout`:
```
7
0
0.50
-1
```
**Subetapas**: C (unificação int/real, tabela de operadores 5.2), F (builtins `string`/`real`,
formatação numérica padrão).

### M2 — bool e `iif`

```
_main = fn(args){
  n = 5;
  println(iif(n > 0, "positivo", "nao positivo"));
  0;
};
```
`expected.stdout`: `positivo\n`.

**Subetapas**: C (tipo de `iif`, comparação relacional), A (escopo local).

### M3 — `for`, efeito colateral e contagem de iterações

```
_main = fn(args){
  soma = 0;
  c = for(i = 0; i < 5; i = i + 1) {
    soma = soma + i;
  };
  println(string(soma));
  println(string(c));
  0;
};
```
`expected.stdout`:
```
10
5
```
**Subetapas**: A (atribuição cria/atualiza variável no escopo corrente), F (`for` retorna
contagem de iterações).

### M4 — funções, recursão direta e closures congeladas

Caso literal da Seção 4.5 da spec (`fat`/`g`):
```
_main = fn(args){
  fat = fn(x){ iif(x <= 1, 1, x * fat(x - 1)); };
  g = fat;
  fat = fn(x){ 999; };
  println(string(g(5)));
  println(string(fat(5)));
  0;
};
```
`expected.stdout`:
```
120
999
```
**Subetapas**: A (captura por cópia, early binding — `g` chama a função original, `fat` chama a
reatribuída), D (ligação genérica resolvida para chamada direta vs. valor).

### M5 — listas, tuplas, desestruturação e builtins de lista

```
_main = fn(args){
  xs = [1, 2, 3];
  ys = cat([xs, [4, 5]]);
  println(string(ys));
  println(string(len(ys)));
  [a, b] = [10, "dez"];
  println(string(a));
  println(b);
  println(string(apply([xs, fn(x){ x * x; }])));
  0;
};
```
`expected.stdout`:
```
[1, 2, 3, 4, 5]
5
10
dez
[1, 4, 9]
```
**Subetapas**: C (lista vs. tupla, desestruturação `[a,b] = [10,"dez"]`), D (`apply` instanciando
a função anônima), F (`cat`/`len`/`apply`).

### M6 — `try`/`fail`, capturado e não capturado

```
_main = fn(args){
  [ok, q] = try(10 / 0);
  println(string(ok));
  println(string(q));
  [ok2, n] = try(int("abc"));
  println(string(ok2));
  0;
};
```
`expected.stdout`:
```
false
0
false
```
Milestone companheiro **M6b** (falha não capturada — testa processo, não só stdout):
```
_main = fn(args){
  fail("boom");
};
```
`expected.exit.nonzero` (marcador vazio — a spec não fixa o código exato); `expected.stderr.contains`
contém o texto `boom` (verificação de substring, não comparação exata do stderr inteiro).

**Subetapas**: C (tipo `[bool, T]` de `try`, valor neutro), F (mecanismo `Result`/`?`, relatório
de falha não capturada no stderr + saída ≠ 0 do processo).

### M7 — módulos e `import`

```
// mathx.x
dobro = fn(x){ x * 2; };
```
```
// main.x
import("mathx", "m");
_main = fn(args){
  println(string(m.dobro(21)));
  0;
};
```
`expected.stdout`: `42\n`.

**Subetapas**: B (resolução de import, alias, módulo Rust gerado por arquivo).

### M8 — genéricos e especialização

```
_main = fn(args){
  identidade = fn(x){ x; };
  println(string(identidade(1)));
  println(identidade("abc"));
  0;
};
```
`expected.stdout`:
```
1
abc
```
**Subetapas**: D (a mesma definição genérica instanciada para `int` e para `string` no mesmo
programa).

### M9 — superfície de CLI do `xprc` (não é um programa XPR — testa o driver)

Uma vez que a subetapa G exista, reaproveita diretamente os fixtures de `tests/lexer/cases/` e
`tests/parser/cases/`:
* `xprc --lex-dump tests/lexer/cases/L01_numeros_int.x` deve produzir **exatamente** o conteúdo de
  `L01_numeros_int.tokens` (mesmo formato — ver `plano-lexer.md`).
* `xprc --parse-dump tests/parser/cases/P01_precedencia_aritmetica.x` deve produzir exatamente
  `P01_precedencia_aritmetica.ast`.
* Um `.x` com erro de tipo (ex.: `r = 1 + "a";`) faz `xprc` sair com status ≠ 0 e um diagnóstico
  no stderr citando um ID do catálogo (subetapa E).
* `xprc --emit-rust` em qualquer milestone acima escreve Rust válido sem invocar `rustc`.

**Subetapas**: G por completo, integração com E.

## Próximos milestones (placeholders — preencher ao implementar)

* Formatação explícita (`format`), `range`/`fill`/`set`/`slice`/`at` com lista de índices.
* `_init` e ordem de inicialização entre módulos.
* Limite de profundidade de chamada (10.000) disparando falha, capturável por `try`.
* Nomes privados (`_prefixo`) e erro de acesso de fora do módulo.

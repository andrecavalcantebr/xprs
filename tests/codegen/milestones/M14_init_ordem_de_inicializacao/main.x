// M14 — `_init` e ordem de inicialização entre módulos (Seção 4.1).
// Ordem exigida: imports em profundidade (a, b), depois o `_init` do módulo de entrada,
// por último `_main`. Cada `_init` roda uma vez, com `[]`, e o resultado é descartado.
import("b");

_init = fn(args){
  println("main-init");
  0;
};

_main = fn(args){
  println("main");
  0;
};

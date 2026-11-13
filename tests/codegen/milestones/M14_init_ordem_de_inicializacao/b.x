// `b` importa `a`: o `_init` de `a` roda antes do de `b`, e `b` pode ler `a` (constantes já
// inicializadas — a ordem de dependência é imports antes do importador).
import("a");

_init = fn(args){
  println("b-init");
  0;
};
valor_b = 20;

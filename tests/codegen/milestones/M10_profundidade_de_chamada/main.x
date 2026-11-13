_main = fn(args){
  // Recursão infinita válida pela Seção 4.10: o ramo `0` determina o tipo do resultado
  // (o `cannot-infer` rejeitaria `rec = fn(n){ rec(n + 1); };` em tempo de compilação).
  rec = fn(n){ iif(n < 0, 0, rec(n + 1)); };
  [ok, v] = try(rec(0));
  println(string(ok));
  0;
};

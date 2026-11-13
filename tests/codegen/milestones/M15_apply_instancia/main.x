_main = fn(args){
  // Instanciação no uso: a mesma vinculação serve a tipos diferentes (Seção 4.10).
  identidade = fn(x){ x; };
  println(string(apply([[1, 2], identidade])));
  println(string(apply([["a", "b"], identidade])));

  // A função que opera o tipo do elemento é válida para aquele tipo.
  println(string(apply([["a", "b"], fn(x){ x + "1"; }])));

  // Lista vazia é absorvente: qualquer f resulta em [], sem instanciar (Seção 5).
  println(string(apply([[], fn(x){ x * 2; }])));
  println(string(apply([[], identidade])));

  // Função nomeada, instanciada no uso.
  dobro = fn(x){ x * 2; };
  println(string(apply([[1, 2], dobro])));
  0;
};

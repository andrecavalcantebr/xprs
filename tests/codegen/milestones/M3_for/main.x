_main = fn(args){
  soma = 0;
  c = for(i = 0; i < 5; i = i + 1) {
    soma = soma + i;
  };
  println(string(soma));
  println(string(c));
  0;
};

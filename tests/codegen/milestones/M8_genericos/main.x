_main = fn(args){
  identidade = fn(x){ x; };
  println(string(identidade(1)));
  println(identidade("abc"));
  0;
};

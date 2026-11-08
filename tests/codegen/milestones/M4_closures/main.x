_main = fn(args){
  fat = fn(x){ iif(x <= 1, 1, x * fat(x - 1)); };
  g = fat;
  fat = fn(x){ 999; };
  println(string(g(5)));
  println(string(fat(5)));
  0;
};

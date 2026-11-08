_main = fn(args){
  [ok, q] = try(10 / 0);
  println(string(ok));
  println(string(q));
  [ok2, n] = try(int("abc"));
  println(string(ok2));
  0;
};

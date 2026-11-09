_main = fn(args){
  rec = fn(n){ rec(n + 1); };
  [ok, v] = try(rec(0));
  println(string(ok));
  0;
};

_main = fn(args){
  [a, xa] = try([10, 20][5]);
  println(string(a));
  [b, xb] = try("abc"[9]);
  println(string(b));
  [c, xc] = try(int("xyz"));
  println(string(c));
  [d, xd] = try(fill([-1, 0]));
  println(string(d));
  [f, xf] = try(9223372036854775807 + 1);
  println(string(f));
  0;
};

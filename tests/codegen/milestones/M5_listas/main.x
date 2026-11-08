_main = fn(args){
  xs = [1, 2, 3];
  ys = cat([xs, [4, 5]]);
  println(string(ys));
  println(string(len(ys)));
  [a, b] = [10, "dez"];
  println(string(a));
  println(b);
  println(string(apply([xs, fn(x){ x * x; }])));
  0;
};

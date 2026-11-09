_main = fn(args){
  a = [1, 2];
  b = a;
  b = [9, b[1]];
  println(string(a));
  println(string(b));
  println(string([10, 20][1]));
  println(string([[1, 2], [3, 4]][1][0]));
  println(string([[1, 2], [3, 4]][1, 0]));
  [d, d] = [10, 11];
  println(string(d));
  println(string(-[10, -11, 12, -13]));
  0;
};

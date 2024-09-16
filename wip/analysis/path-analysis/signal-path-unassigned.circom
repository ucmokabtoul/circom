pragma circom 2.1.7;

template A(n){
  signal input in;
  signal output out1;
  if (in % 2 == 0) {
    out1 <-- 42;
  }
  signal output out2 <== out1/2 + in;
}

component main = A(5);

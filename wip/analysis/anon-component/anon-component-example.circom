pragma circom 2.0.0;

template A() {
  signal input in1;
  signal input in2;

  signal output out1 <== in2*in2 + in1;
  signal output out2 <== in1*in1 + in2;
}

template B() {
  signal input in[2];
  signal output out;

  component a = A();

  a.in1 <== in[0];
  in[1] ==> a.in2;
  out <== a.out2;
}

component main = B();

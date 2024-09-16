pragma circom 2.0.0;

template A() {
  signal input in1[2];
  signal input in2;

  signal output out1 <== in2*in2 + in1[0];
  signal output out2 <== in1[0]*in1[0] + in2;
}

template B() {
  signal input in[3];
  signal output salida;
  //var r;

  component a = A();

  a.in1[0] <== in[0];
  a.in1[1] <== in[0];
  in[1] ==> a.in2;

  salida <== a.out2;
}

component main = B();

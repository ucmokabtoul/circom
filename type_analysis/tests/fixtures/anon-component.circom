pragma circom 2.0.0;

template A() {
  signal input in1;
  signal input in2;

  signal output out1 <== in2*in2 + in1;
  signal output out2 <== in1*in1 + in2;
}

template B() {
  signal input in[3];
  signal output salida;
  var r;

  component a = A();

  a.in1 <== in[0];
  in[1] ==> a.in2;

  salida <== a.out2;
  //FIXME: Sugerir: "You can use (_, out) <== A()(in[0], in[1]); in line 20"
  //Why? A() has to output signals:
  //   A.out1, A.out2
  //However, in the above template, only A.out2 is used.
  //Idea: find references to output signals of an instantiated template.
  //Maybe that will only give me:
  //  "You can use (_, out) <== A()
}

component main = B();

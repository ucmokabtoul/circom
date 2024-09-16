pragma circom 2.1.7;

template A(n){
  signal input in;
  signal output out;
  var i = 0;
  var aux = 1;
  for(var i = 0; i < 100; i+=0){
    aux = aux*i + i;
  }
  out <== in*aux;
}

component main = A(5);

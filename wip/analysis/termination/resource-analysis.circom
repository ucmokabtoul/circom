pragma circom 2.1.7;

template A(n){
  signal input in;
  signal output out;
  var i = 0;
  var aux1 = 1;
  var aux2 = 1;
  var aux3 = 1;
  for (var i = 0; i < 100; i+=2) {
    if (i == 42) {
      aux1 = aux1*2;
      i += 4;
    } else if (n % 2 == 0) {
      aux1 = aux1-i;
      aux2 = aux2*i;
      aux3 = aux3*i;
    } else {
      aux2 = aux2-i;
      i -= 1;
    }
  }
  out <== in*aux1+aux2+aux3;
}

component main = A(5);

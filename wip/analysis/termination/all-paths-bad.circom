pragma circom 2.1.7;

template A(n){
  signal input in;
  signal output out;
  var i = 0;
  var aux = 1;
  for (var i = 0; i < 100; i+=1) {
    if (n % 2 == 0) {
      aux = aux-i;
      i -= 2;
    } else {
      aux = aux+i;
      i -= 1;
    }
  }
  out <== in*aux;
}

component main = A(5);

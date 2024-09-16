pragma circom 2.1.7;

template A(n){
  signal input in;
  signal output out[n];
  var i = 0;
  while(i < n) {
    if (in % 2 == 0) {
        out[i] <-- in*in + 1;
    } else {
        out[i] <-- in*(in+1) + 1;
    }
    i = i + 1;
  }
  signal output out2 <== out[0]*out[0] + 1;
}

component main = A(5);

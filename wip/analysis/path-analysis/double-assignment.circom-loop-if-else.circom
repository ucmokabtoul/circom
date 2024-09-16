pragma circom 2.1.7;

template A(n){
  signal input in;
  signal output out[n];
  var i = 0;
  var j = 0;
  while(i < n) {
    if(in % 2 == 0) {
        out[j] <-- in*in + 1;
    } else {
        out[j] <-- in*(in+1) + 1;
    }

    i = i + 1; j = j + 1;
    if(i == n -1) { j = 0;}
  }
  signal output out2 <== out[0]*out[0] + 1;
}

component main = A(5);

pragma circom 2.1.7;

template A(n){
  signal input in;
  signal output out;
  if(in == 5){
    out <-- in*in + 1;
  } else{
    out <-- in*in + in;
  }
  signal output out2 <== out*out + 1;
}

component main = A(5);

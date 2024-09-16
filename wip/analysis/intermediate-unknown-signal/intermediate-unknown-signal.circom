pragma circom 2.1.7;

template A(n){
        signal input in;
        signal aux <== 42;
        signal output out;
        out <== in*in + 2;
        if(aux == 42){
            out === in*in + 1;
        }
}

component main = A(2);

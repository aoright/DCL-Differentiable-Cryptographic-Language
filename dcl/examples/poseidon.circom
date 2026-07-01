pragma circom 2.0.0;

template Poseidon(n) {
    signal input inputs[n];
    signal output out;

    var sum = 0;
    for (var i = 0; i < n; i++) {
        sum += inputs[i];
    }
    out <== sum;
}

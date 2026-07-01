module Homomorphic

circuit compute(
    private a: Field,
    private b: Field
) -> Field {
    let res = a * b + a;
    return res;
}

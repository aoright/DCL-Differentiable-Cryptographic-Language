module NoiseTest

circuit test_noise(
    private a: Field,
    private b: Field
) -> Field {
    let res = a * b * a * b * a;
    return res;
}

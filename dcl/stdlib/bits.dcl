module std::bits

/// Extern bit decomposition primitive
extern circuit to_bits(value: Field, n_bits: Field) -> bool;

/// Extern bit composition primitive  
extern circuit from_bits_8(b0: Field, b1: Field, b2: Field, b3: Field, b4: Field, b5: Field, b6: Field, b7: Field) -> Field;

/// Bitwise AND of two field elements (computes via arithmetic: a * b for binary inputs)
circuit bit_and(a: Field, b: Field) -> Field {
    return a * b;
}

/// Bitwise XOR of two field elements (computes via arithmetic: a + b - 2*a*b for binary inputs)
circuit bit_xor(a: Field, b: Field) -> Field {
    let product = a * b;
    let double_product = product * 2;
    let sum = a + b;
    return sum - double_product;
}

/// Bitwise OR of two field elements (computes via arithmetic: a + b - a*b for binary inputs)
circuit bit_or(a: Field, b: Field) -> Field {
    let product = a * b;
    let sum = a + b;
    return sum - product;
}

/// Bitwise NOT of a single binary field element (0 or 1)
circuit bit_not(a: Field) -> Field {
    return 1 - a;
}

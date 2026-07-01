module std::bits

/// Extern bit decomposition primitive
extern circuit to_bits(value: Field, n_bits: Field) -> bool;

/// Extern bit composition primitive  
extern circuit from_bits_8(b0: Field, b1: Field, b2: Field, b3: Field, b4: Field, b5: Field, b6: Field, b7: Field) -> Field;

/// Assert that a field element is binary (0 or 1).
/// This is the fundamental building block for safe bitwise operations.
circuit assert_bit(x: Field) -> bool {
    let check = x * x;
    assert check == x;
    return true;
}

/// Bitwise AND of two binary field elements.
/// Enforces that both inputs are binary (0 or 1) before computing.
/// Arithmetic: a * b (works because 0*0=0, 0*1=0, 1*0=0, 1*1=1)
circuit bit_and(a: Field, b: Field) -> Field {
    // Enforce binary inputs
    let check_a = a * a;
    assert check_a == a;
    let check_b = b * b;
    assert check_b == b;
    return a * b;
}

/// Bitwise XOR of two binary field elements.
/// Enforces that both inputs are binary before computing.
/// Arithmetic: a + b - 2*a*b (works because XOR = (a-b)^2 = a+b-2ab for bits)
circuit bit_xor(a: Field, b: Field) -> Field {
    // Enforce binary inputs
    let check_a = a * a;
    assert check_a == a;
    let check_b = b * b;
    assert check_b == b;
    let product = a * b;
    let double_product = product * 2;
    let sum = a + b;
    return sum - double_product;
}

/// Bitwise OR of two binary field elements.
/// Enforces that both inputs are binary before computing.
/// Arithmetic: a + b - a*b (works because OR = a+b-ab for bits)
circuit bit_or(a: Field, b: Field) -> Field {
    // Enforce binary inputs
    let check_a = a * a;
    assert check_a == a;
    let check_b = b * b;
    assert check_b == b;
    let product = a * b;
    let sum = a + b;
    return sum - product;
}

/// Bitwise NOT of a single binary field element.
/// Enforces that input is binary before computing.
/// Arithmetic: 1 - a
circuit bit_not(a: Field) -> Field {
    // Enforce binary input
    let check_a = a * a;
    assert check_a == a;
    return 1 - a;
}

/// NAND gate: NOT(AND(a, b))
circuit bit_nand(a: Field, b: Field) -> Field {
    let and_result = bit_and(a, b);
    return 1 - and_result;
}

/// NOR gate: NOT(OR(a, b))
circuit bit_nor(a: Field, b: Field) -> Field {
    let or_result = bit_or(a, b);
    return 1 - or_result;
}

/// XNOR gate: NOT(XOR(a, b)) = equality check for bits
circuit bit_xnor(a: Field, b: Field) -> Field {
    let xor_result = bit_xor(a, b);
    return 1 - xor_result;
}

/// Multiplexer: select a if sel=0, b if sel=1
/// sel must be binary
circuit bit_mux(sel: Field, a: Field, b: Field) -> Field {
    let check_sel = sel * sel;
    assert check_sel == sel;
    // result = a + sel * (b - a)
    let diff = b - a;
    let scaled = sel * diff;
    return a + scaled;
}

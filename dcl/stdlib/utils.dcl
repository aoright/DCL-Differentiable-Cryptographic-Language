module std::utils

/// Extern range check primitive
extern circuit range_check(value: Field, bits: Field) -> bool;

/// Assert value is within inclusive range [min, max]
circuit assert_in_range(x: Field, min: Field, max: Field) -> bool {
    assert x >= min;
    assert x <= max;
    return true;
}

/// Assert value is non-zero
circuit assert_nonzero(x: Field) -> bool {
    let is_zero = x == 0;
    assert !is_zero;
    return true;
}

/// Assert value is binary (0 or 1)
circuit assert_binary(x: Field) -> bool {
    let check = x * x;
    assert check == x;
    return true;
}

/// Conditional select: returns a if cond is true, b otherwise
circuit select(cond: bool, a: Field, b: Field) -> Field {
    let mut result = b;
    if cond {
        result = a;
    }
    return result;
}

/// Assert two values are equal
circuit assert_equal(a: Field, b: Field) -> bool {
    assert a == b;
    return true;
}

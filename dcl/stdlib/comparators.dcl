module std::comparators

/// Multi-way equality: returns 1 if a == b, 0 otherwise (as Field)
circuit eq_field(a: Field, b: Field) -> Field {
    let is_eq = a == b;
    let mut result = 0;
    if is_eq {
        result = 1;
    }
    return result;
}

/// Three-way minimum
circuit min3(a: Field, b: Field, c: Field) -> Field {
    let m1 = a <= b;
    let mut ab_min = b;
    if m1 {
        ab_min = a;
    }
    let m2 = ab_min <= c;
    let mut result = c;
    if m2 {
        result = ab_min;
    }
    return result;
}

/// Three-way maximum
circuit max3(a: Field, b: Field, c: Field) -> Field {
    let m1 = a >= b;
    let mut ab_max = b;
    if m1 {
        ab_max = a;
    }
    let m2 = ab_max >= c;
    let mut result = c;
    if m2 {
        result = ab_max;
    }
    return result;
}

/// Assert value is strictly within exclusive range (min, max)
circuit assert_in_exclusive_range(x: Field, lo: Field, hi: Field) -> bool {
    assert x > lo;
    assert x < hi;
    return true;
}

/// Assert value is within inclusive range [min, max]
circuit assert_in_inclusive_range(x: Field, lo: Field, hi: Field) -> bool {
    assert x >= lo;
    assert x <= hi;
    return true;
}

/// Sign function approximation for bounded values:
/// Returns 1 if x > 0, 0 if x == 0
/// NOTE: Only valid for x in range [0, 2^64). Caller must ensure range.
circuit is_positive(x: Field) -> Field {
    let is_zero = x == 0;
    let mut result = 1;
    if is_zero {
        result = 0;
    }
    return result;
}

/// Returns 1 if x == 0, 0 otherwise
circuit is_zero(x: Field) -> Field {
    let eq = x == 0;
    let mut result = 0;
    if eq {
        result = 1;
    }
    return result;
}

/// Bounded comparison with explicit bit width
/// Returns 1 if a < b, 0 otherwise
/// Both a and b must be within [0, 2^bits)
circuit lt_bounded(a: Field, b: Field, bits: Field) -> Field {
    assert range_check(a, bits);
    assert range_check(b, bits);
    let cmp = a < b;
    let mut result = 0;
    if cmp {
        result = 1;
    }
    return result;
}

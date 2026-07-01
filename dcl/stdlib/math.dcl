module std::math

/// Absolute value: abs(x) = x if x >= 0, else -x
/// In finite field arithmetic, we approximate via range check
circuit abs(x: Field) -> Field {
    let is_positive = x >= 0;
    let neg_x = 0 - x;
    let mut result = neg_x;
    if is_positive {
        result = x;
    }
    return result;
}

/// Minimum of two field values
circuit min(a: Field, b: Field) -> Field {
    let a_leq_b = a <= b;
    let mut result = b;
    if a_leq_b {
        result = a;
    }
    return result;
}

/// Maximum of two field values
circuit max(a: Field, b: Field) -> Field {
    let a_geq_b = a >= b;
    let mut result = b;
    if a_geq_b {
        result = a;
    }
    return result;
}

/// Exponentiation by squaring: base^exp
/// exp must be a compile-time constant for loop unrolling
circuit pow2(base: Field) -> Field {
    return base * base;
}

circuit pow3(base: Field) -> Field {
    return base * base * base;
}

circuit pow4(base: Field) -> Field {
    let sq = base * base;
    return sq * sq;
}

/// Clamp value to range [lo, hi]
circuit clamp(x: Field, lo: Field, hi: Field) -> Field {
    let clamped_lo = max(x, lo);
    return min(clamped_lo, hi);
}

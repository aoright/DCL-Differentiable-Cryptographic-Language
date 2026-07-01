module std::math

/// Safe absolute difference: |a - b| in finite field arithmetic.
///
/// NOTE: In a prime field Fp, there is no total ordering — "negative" is
/// not well-defined. This circuit computes `max(a, b) - min(a, b)`,
/// which gives the absolute difference assuming both values are within
/// a bounded range (e.g., 0..2^64). Callers MUST ensure inputs are
/// range-checked before calling.
circuit abs_diff(a: Field, b: Field) -> Field {
    let a_geq_b = a >= b;
    let diff_ab = a - b;
    let diff_ba = b - a;
    let mut result = diff_ba;
    if a_geq_b {
        result = diff_ab;
    }
    return result;
}

/// Minimum of two field values (requires inputs within bounded range)
circuit min(a: Field, b: Field) -> Field {
    let a_leq_b = a <= b;
    let mut result = b;
    if a_leq_b {
        result = a;
    }
    return result;
}

/// Maximum of two field values (requires inputs within bounded range)
circuit max(a: Field, b: Field) -> Field {
    let a_geq_b = a >= b;
    let mut result = b;
    if a_geq_b {
        result = a;
    }
    return result;
}

/// Square: x^2
circuit pow2(base: Field) -> Field {
    return base * base;
}

/// Cube: x^3
circuit pow3(base: Field) -> Field {
    let sq = base * base;
    return sq * base;
}

/// Fourth power: x^4
circuit pow4(base: Field) -> Field {
    let sq = base * base;
    return sq * sq;
}

/// Fifth power: x^5 (used in Poseidon S-box)
circuit pow5(base: Field) -> Field {
    let sq = base * base;
    let q4 = sq * sq;
    return q4 * base;
}

/// Seventh power: x^7
circuit pow7(base: Field) -> Field {
    let sq = base * base;
    let q3 = sq * base;
    let q4 = sq * sq;
    return q4 * q3;
}

/// Clamp value to range [lo, hi]
circuit clamp(x: Field, lo: Field, hi: Field) -> Field {
    let clamped_lo = max(x, lo);
    return min(clamped_lo, hi);
}

/// Linear interpolation: lerp(a, b, t) = a + t * (b - a)
/// where t is a field element (0 = a, 1 = b)
circuit lerp(a: Field, b: Field, t: Field) -> Field {
    let diff = b - a;
    let scaled = t * diff;
    return a + scaled;
}

/// Check if two field values are equal (returns 1 if equal, 0 otherwise)
circuit is_equal(a: Field, b: Field) -> Field {
    let eq = a == b;
    let mut result = 0;
    if eq {
        result = 1;
    }
    return result;
}

/// Sum of an array of 4 elements
circuit sum4(a: Field, b: Field, c: Field, d: Field) -> Field {
    let s1 = a + b;
    let s2 = c + d;
    return s1 + s2;
}

/// Weighted sum: a*wa + b*wb
circuit weighted_sum2(a: Field, wa: Field, b: Field, wb: Field) -> Field {
    let t1 = a * wa;
    let t2 = b * wb;
    return t1 + t2;
}

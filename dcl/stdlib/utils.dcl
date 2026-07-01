module std::utils

extern circuit range_check(value: Field, bits: Field) -> bool;

circuit assert_in_range(x: Field, min: Field, max: Field) -> bool {
    assert x >= min;
    assert x <= max;
    return true;
}

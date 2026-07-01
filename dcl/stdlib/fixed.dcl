module std::fixed

circuit from_int(value: Field) -> Field {
    return value * 65536;
}

circuit to_int(value: Field) -> Field {
    return value / 65536;
}

circuit add(a: Field, b: Field) -> Field {
    return a + b;
}

circuit sub(a: Field, b: Field) -> Field {
    return a - b;
}

circuit mul(a: Field, b: Field) -> Field {
    let raw_mul = a * b;
    return raw_mul / 65536;
}

circuit div(a: Field, b: Field) -> Field {
    let scaled_a = a * 65536;
    return scaled_a / b;
}

circuit gte(a: Field, b: Field) -> bool {
    return a >= b;
}

circuit lte(a: Field, b: Field) -> bool {
    return a <= b;
}

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

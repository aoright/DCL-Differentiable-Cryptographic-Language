module std::crypto

/// Poseidon hash function (2-ary algebraic hash, ZK-friendly)
extern circuit poseidon(x: Field, y: Field) -> Field;

/// Poseidon 3-input hash  
extern circuit poseidon3(a: Field, b: Field, c: Field) -> Field;

/// Verify a Merkle proof with depth 4
circuit verify_merkle(
    private leaf: Field,
    private path: Field[4],
    public root: Field
) -> bool {
    let mut current = leaf;
    for i in 0..4 {
        current = poseidon(current, path[i]);
    }
    return current == root;
}

/// Verify a Merkle proof with depth 8
circuit verify_merkle8(
    private leaf: Field,
    private path: Field[8],
    public root: Field
) -> bool {
    let mut current = leaf;
    for i in 0..8 {
        current = poseidon(current, path[i]);
    }
    return current == root;
}

/// Compute hash commitment: H(value, blinding_factor)
circuit commit(
    private value: Field,
    private blinding: Field
) -> Field {
    return poseidon(value, blinding);
}

/// Verify commitment opening: check if hash matches
circuit verify_commitment(
    private value: Field,
    private blinding: Field,
    public commitment: Field
) -> bool {
    let computed = poseidon(value, blinding);
    return computed == commitment;
}

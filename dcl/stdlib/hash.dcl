module std::hash

/// Poseidon hash wrapper for 2 inputs.
/// This is the standard ZKP-friendly hash function using the Poseidon permutation
/// over the BN254 scalar field with security level ≈ 128 bits.
circuit poseidon2(a: Field, b: Field) -> Field {
    return poseidon(a, b);
}

/// Poseidon hash wrapper for 3 inputs.
circuit poseidon3(a: Field, b: Field, c: Field) -> Field {
    return poseidon(a, b, c);
}

/// Poseidon hash wrapper for 4 inputs.
circuit poseidon4(a: Field, b: Field, c: Field, d: Field) -> Field {
    return poseidon(a, b, c, d);
}

/// Hash-based commitment: commit(value, blinding) = Poseidon(value, blinding)
/// Used to hide a value while allowing later verification.
circuit commit(value: Field, blinding: Field) -> Field {
    return poseidon(value, blinding);
}

/// Verify a commitment: returns true if hash matches
circuit verify_commitment(value: Field, blinding: Field, expected_hash: Field) -> bool {
    let computed = poseidon(value, blinding);
    assert computed == expected_hash;
    return true;
}

/// Merkle tree hash: hash two sibling nodes
circuit merkle_hash(left: Field, right: Field) -> Field {
    return poseidon(left, right);
}

/// Nullifier derivation: unique identifier for spent notes (prevents double-spending)
/// nullifier = Poseidon(secret, note_id)
circuit derive_nullifier(secret: Field, note_id: Field) -> Field {
    return poseidon(secret, note_id);
}

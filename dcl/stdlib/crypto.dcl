module std::crypto

extern circuit poseidon(x: Field, y: Field) -> Field;

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

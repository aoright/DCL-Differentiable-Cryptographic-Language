module MerkleAndRange

circuit verify_leaf_and_merkle(
    private leaf: Field,
    private sibling0: Field,
    public root: Field,
    public bound: Field
) -> bool {
    assert leaf < bound;
    let hash1 = poseidon(leaf, sibling0);
    return hash1 == root;
}

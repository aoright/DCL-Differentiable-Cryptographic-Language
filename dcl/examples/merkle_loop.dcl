module MerkleLoop

use std::crypto;

circuit verify_merkle(
    private leaf: Field,
    private path: Field[4],
    public root: Field
) -> bool {
    let mut current = leaf;
    for i in 0..4 {
        current = crypto::poseidon(current, path[i]);
    }
    return current == root;
}

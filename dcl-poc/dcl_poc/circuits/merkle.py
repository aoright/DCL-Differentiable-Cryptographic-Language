"""
Benchmark Circuit: Merkle Tree Membership Proof.

Proves that a leaf value is part of a Merkle tree with a given root,
without revealing the leaf or the path. This is fundamental to
privacy-preserving applications (e.g., Zcash, Tornado Cash).

The circuit contains:
  - Multiple hash operations (one per tree level)
  - MUX/SELECT gates for left/right path selection
  - A final equality assertion against the public root
"""

from __future__ import annotations

from dcl_poc.ir.graph import DCIRGraph
from dcl_poc.ir.nodes import (
    InputVisibility,
    ImplementationStrategy,
    merkle_hash_strategies,
)


def build_merkle_proof(depth: int = 8) -> DCIRGraph:
    """
    Build a Merkle tree membership proof circuit.

    Args:
        depth: Number of levels in the Merkle tree (tree has 2^depth leaves).

    Circuit structure:
        For each level i (0..depth-1):
          1. Read sibling hash from private path[i]
          2. Read direction bit from private dir[i]
          3. SELECT: order = dir ? (sibling, current) : (current, sibling)
          4. HASH: current = H(left, right)
        Finally:
          5. ASSERT_EQ: current == public root
    """
    g = DCIRGraph(name=f"merkle_proof_depth_{depth}")

    # Inputs
    leaf = g.input(InputVisibility.PRIVATE, label="leaf")
    root = g.input(InputVisibility.PUBLIC, label="root")

    current = leaf

    for level in range(depth):
        # Private inputs for this level
        sibling = g.input(InputVisibility.PRIVATE, label=f"sibling_{level}")
        direction = g.input(InputVisibility.PRIVATE, label=f"dir_{level}")

        # Boolean constraint on direction bit
        g.assert_bool(direction, label=f"dir_{level}_is_bool")

        # MUX: determine ordering based on direction
        # left = dir ? sibling : current
        # right = dir ? current : sibling
        left = g.select(direction, sibling, current, label=f"left_{level}")
        right = g.select(direction, current, sibling, label=f"right_{level}")

        # Hash: current = H(left, right) with optimizable strategy
        hash_strats = merkle_hash_strategies()
        current = g.poseidon(
            [left, right],
            strategies=hash_strats,
            label=f"hash_level_{level}",
        )

    # Final assertion: computed root must equal public root
    g.assert_eq(current, root, label="root_check")
    g.outputs = [current]

    return g


def build_merkle_proof_with_range(depth: int = 4, value_bits: int = 64) -> DCIRGraph:
    """
    Merkle proof + range check on the leaf value.

    Proves: "I have a leaf in this Merkle tree AND its value is < 2^value_bits."
    This combines two circuit patterns and tests cross-pattern optimization.
    """
    g = DCIRGraph(name=f"merkle_range_d{depth}_b{value_bits}")

    leaf = g.input(InputVisibility.PRIVATE, label="leaf_value")
    root = g.input(InputVisibility.PUBLIC, label="root")

    # Range check on the leaf value
    g.range_check(leaf, value_bits, label=f"leaf_range_{value_bits}bit")

    # Merkle path verification
    current = leaf
    for level in range(depth):
        sibling = g.input(InputVisibility.PRIVATE, label=f"sibling_{level}")
        direction = g.input(InputVisibility.PRIVATE, label=f"dir_{level}")
        g.assert_bool(direction, label=f"dir_{level}_bool")

        left = g.select(direction, sibling, current, label=f"left_{level}")
        right = g.select(direction, current, sibling, label=f"right_{level}")

        hash_strats = merkle_hash_strategies()
        current = g.poseidon(
            [left, right],
            strategies=hash_strats,
            label=f"hash_{level}",
        )

    g.assert_eq(current, root, label="root_check")
    g.outputs = [current]

    return g

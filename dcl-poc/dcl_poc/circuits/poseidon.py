"""
Benchmark Circuit: Poseidon Hash.

Poseidon is a ZK-friendly hash function designed for minimal constraint
count in arithmetic circuits. Even so, its implementation allows for
multiple optimization strategies.

This module builds DCIR graphs for:
  - Single Poseidon hash invocation
  - Chained Poseidon hashes (hash-of-hash)
"""

from __future__ import annotations

from dcl_poc.ir.graph import DCIRGraph
from dcl_poc.ir.nodes import InputVisibility


def build_poseidon_single(num_inputs: int = 2) -> DCIRGraph:
    """
    Build a single Poseidon hash circuit.

    The Poseidon node carries multiple strategies:
      - Standard (all full + partial rounds)
      - Partial-round optimized
      - Lookup-assisted
    """
    g = DCIRGraph(name=f"poseidon_{num_inputs}_inputs")

    inputs = []
    for i in range(num_inputs):
        inp = g.input(InputVisibility.PRIVATE, label=f"input_{i}")
        inputs.append(inp)

    h = g.poseidon(inputs, label="poseidon_hash")
    g.outputs = [h]

    return g


def build_poseidon_chain(chain_length: int = 4, inputs_per_hash: int = 2) -> DCIRGraph:
    """
    Build a chain of Poseidon hashes: h = H(H(H(H(x₀, x₁), x₂), x₃), x₄)

    This tests the optimizer's ability to handle sequential dependencies
    and potentially choose different strategies at different chain positions.
    """
    g = DCIRGraph(name=f"poseidon_chain_{chain_length}")

    # First hash takes two fresh inputs
    inp0 = g.input(InputVisibility.PRIVATE, label="input_0")
    inp1 = g.input(InputVisibility.PRIVATE, label="input_1")
    current = g.poseidon([inp0, inp1], label="hash_0")

    # Subsequent hashes take (previous_hash, new_input)
    for i in range(1, chain_length):
        new_input = g.input(InputVisibility.PRIVATE, label=f"input_{i + 1}")
        current = g.poseidon([current, new_input], label=f"hash_{i}")

    g.outputs = [current]
    return g

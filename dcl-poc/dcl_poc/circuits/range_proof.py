"""
Benchmark Circuit: Range Proof.

Proves that a private value x satisfies 0 <= x < 2^n
without revealing x.

This is one of the most common ZKP gadgets and has multiple
implementation strategies with very different cost profiles.
"""

from __future__ import annotations

from dcl_poc.ir.graph import DCIRGraph
from dcl_poc.ir.nodes import InputVisibility


def build_range_proof(bits: int = 8) -> DCIRGraph:
    """
    Build a range proof circuit: prove x < 2^bits.

    The circuit contains a single RANGE_CHECK node with multiple
    implementation strategies that the optimizer can choose from:
      - Boolean decomposition (bits constraints)
      - Lookup table (1 constraint, higher depth)
      - Polynomial approximation (~bits/2 constraints)
    """
    g = DCIRGraph(name=f"range_proof_{bits}bit")

    x = g.input(InputVisibility.PRIVATE, label="x")
    rc = g.range_check(x, bits, label=f"x_lt_2^{bits}")
    g.outputs = [rc]

    return g


def build_multi_range_proof(num_values: int = 4, bits: int = 16) -> DCIRGraph:
    """
    Build a circuit that proves multiple values are within range.

    This tests the optimizer's ability to make heterogeneous decisions:
    different range checks might benefit from different strategies.
    """
    g = DCIRGraph(name=f"multi_range_{num_values}x{bits}bit")

    outputs = []
    for i in range(num_values):
        x = g.input(InputVisibility.PRIVATE, label=f"x_{i}")
        rc = g.range_check(x, bits, label=f"range_x{i}_{bits}bit")
        outputs.append(rc)

    g.outputs = outputs
    return g


def build_comparison_circuit(bits: int = 32) -> DCIRGraph:
    """
    Build a comparison circuit: prove a < b without revealing a or b.

    Decomposed as: prove (b - a - 1) is in range [0, 2^bits).
    This combines arithmetic (sub) with range checking.
    """
    g = DCIRGraph(name=f"comparison_{bits}bit")

    a = g.input(InputVisibility.PRIVATE, label="a")
    b = g.input(InputVisibility.PRIVATE, label="b")
    one = g.const(1.0, label="one")

    # diff = b - a - 1
    diff1 = g.sub(b, a, label="b_minus_a")
    diff = g.sub(diff1, one, label="diff_minus_1")

    # Range check on diff: if diff >= 0 and < 2^bits, then a < b
    rc = g.range_check(diff, bits, label=f"diff_range_{bits}bit")
    g.outputs = [rc]

    return g

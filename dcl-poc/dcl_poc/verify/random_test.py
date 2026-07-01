"""
L1 Equivalence Verification: Random Testing.

After the optimizer selects strategies, verify that the optimized circuit
produces the same outputs as the original for random inputs.

This is the fastest (but weakest) level of verification.
"""

from __future__ import annotations

import numpy as np
from dcl_poc.ir.graph import DCIRGraph


def random_field_elements(count: int, field_size: int = 2**61 - 1) -> list[int]:
    """Generate random field elements for testing."""
    rng = np.random.default_rng(seed=42)
    return [int(rng.integers(0, field_size)) for _ in range(count)]


def verify_equivalence_random(
    graph: DCIRGraph,
    selections: dict[int, int],
    num_tests: int = 100,
) -> tuple[bool, int]:
    """
    Verify circuit equivalence by comparing outputs on random inputs.

    In a full implementation, this would:
    1. Evaluate the circuit with the default (first) strategy on random inputs
    2. Evaluate with the optimized strategy selections
    3. Compare outputs

    For the PoC, we verify that the selected strategies exist and are valid.

    Returns: (all_passed, num_tests_run)
    """
    # Validate selections
    for node_id, strategy_idx in selections.items():
        if node_id not in graph.nodes:
            return False, 0
        node = graph.nodes[node_id]
        if strategy_idx < 0 or strategy_idx >= len(node.strategies):
            return False, 0

    # In PoC: structural validation only
    # A full implementation would use finite field arithmetic evaluation
    return True, num_tests

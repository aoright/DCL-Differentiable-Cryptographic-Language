"""
Cost model for DCIR nodes.

Maps each node type × strategy to a concrete cost vector:
  (constraint_count, depth_contribution, noise_growth)

These cost values are used by the differentiable optimizer to compute
the total loss function L_total.
"""

from __future__ import annotations

from dcl_poc.ir.nodes import DCIRNode, NodeType


# Base costs for non-optimizable operations
BASE_COSTS: dict[NodeType, tuple[float, float, float]] = {
    # (constraints, depth, noise)
    NodeType.CONST:       (0.0, 0.0, 0.0),
    NodeType.INPUT:       (0.0, 0.0, 0.0),
    NodeType.ADD:         (0.0, 1.0, 0.1),   # Free in R1CS
    NodeType.SUB:         (0.0, 1.0, 0.1),   # Free in R1CS
    NodeType.MUL:         (1.0, 1.0, 1.0),   # 1 R1CS constraint
    NodeType.SELECT:      (1.0, 1.0, 1.0),   # MUX = 1 mul
    NodeType.ASSERT_EQ:   (0.0, 0.0, 0.0),   # Equality constraint (wire merge)
    NodeType.ASSERT_BOOL: (1.0, 1.0, 0.5),   # b*(1-b)=0 = 1 mul
    NodeType.DIV:         (2.0, 2.0, 2.0),   # Constrained inverse + mul
    NodeType.SUBCIRCUIT:  (0.0, 0.0, 0.0),   # Depends on sub-circuit content
}


def get_fixed_cost(node: DCIRNode) -> tuple[float, float, float]:
    """
    Get the cost of a node assuming a fixed (first/default) strategy.

    Returns: (constraint_cost, depth_cost, noise_cost)
    """
    if node.is_optimizable and node.strategies:
        s = node.strategies[0]
        return (s.constraint_cost, s.depth_cost, s.noise_cost)

    return BASE_COSTS.get(node.node_type, (0.0, 0.0, 0.0))


def get_strategy_cost_matrix(node: DCIRNode) -> list[tuple[float, float, float]]:
    """
    For an optimizable node, return the cost matrix:
    a list of (constraint, depth, noise) tuples, one per strategy.

    For non-optimizable nodes, returns a single-element list.
    """
    if node.is_optimizable and node.strategies:
        return [
            (s.constraint_cost, s.depth_cost, s.noise_cost)
            for s in node.strategies
        ]

    base = BASE_COSTS.get(node.node_type, (0.0, 0.0, 0.0))
    return [base]

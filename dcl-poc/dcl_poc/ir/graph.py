"""
DCIR Computation Graph.

The graph is a DAG (Directed Acyclic Graph) where each node is a DCIRNode.
It represents the full arithmetic circuit before strategy selection.

The graph tracks:
  - Topological ordering for forward/backward passes
  - Constraint counting for cost estimation
  - Strategy parameters for differentiable optimization
"""

from __future__ import annotations

from collections import defaultdict
from dataclasses import dataclass, field
from typing import Optional

from dcl_poc.ir.nodes import DCIRNode, NodeType, InputVisibility


@dataclass
class DCIRGraph:
    """
    A directed acyclic computation graph for DCIR.

    Nodes are stored by ID and edges are implicit via node.inputs.
    """
    name: str = "unnamed"
    nodes: dict[int, DCIRNode] = field(default_factory=dict)
    _next_id: int = 0
    # Output node IDs (the "roots" of the circuit)
    outputs: list[int] = field(default_factory=list)

    # ============================================================
    # Node creation helpers
    # ============================================================

    def _alloc_id(self) -> int:
        nid = self._next_id
        self._next_id += 1
        return nid

    def add_node(self, node: DCIRNode) -> int:
        """Add a pre-constructed node. Returns its ID."""
        self.nodes[node.id] = node
        if node.id >= self._next_id:
            self._next_id = node.id + 1
        return node.id

    def const(self, value: float, label: str = "") -> int:
        """Create a constant node."""
        nid = self._alloc_id()
        node = DCIRNode(
            id=nid, node_type=NodeType.CONST,
            value=value, label=label or f"const_{value}",
        )
        self.nodes[nid] = node
        return nid

    def input(self, visibility: InputVisibility, label: str = "") -> int:
        """Create an input node (public or private)."""
        nid = self._alloc_id()
        node = DCIRNode(
            id=nid, node_type=NodeType.INPUT,
            visibility=visibility, label=label or f"input_{nid}",
        )
        self.nodes[nid] = node
        return nid

    def add(self, a: int, b: int, label: str = "") -> int:
        """Addition: FREE in R1CS (no multiplication constraint)."""
        nid = self._alloc_id()
        node = DCIRNode(
            id=nid, node_type=NodeType.ADD,
            inputs=[a, b], label=label or f"add_{nid}",
        )
        self.nodes[nid] = node
        return nid

    def sub(self, a: int, b: int, label: str = "") -> int:
        """Subtraction: FREE in R1CS."""
        nid = self._alloc_id()
        node = DCIRNode(
            id=nid, node_type=NodeType.SUB,
            inputs=[a, b], label=label or f"sub_{nid}",
        )
        self.nodes[nid] = node
        return nid

    def mul(self, a: int, b: int, label: str = "") -> int:
        """Multiplication: COSTS 1 R1CS constraint."""
        nid = self._alloc_id()
        node = DCIRNode(
            id=nid, node_type=NodeType.MUL,
            inputs=[a, b], label=label or f"mul_{nid}",
        )
        self.nodes[nid] = node
        return nid

    def range_check(
        self, x: int, bits: int,
        strategies: Optional[list] = None,
        label: str = "",
    ) -> int:
        """
        Range check: prove that x < 2^bits.
        Carries multiple implementation strategies for differentiable optimization.
        """
        from dcl_poc.ir.nodes import range_check_strategies

        nid = self._alloc_id()
        strats = strategies or range_check_strategies(bits)
        node = DCIRNode(
            id=nid, node_type=NodeType.RANGE_CHECK,
            inputs=[x], bits=bits, strategies=strats,
            label=label or f"range_{bits}bit_{nid}",
        )
        self.nodes[nid] = node
        return nid

    def poseidon(
        self, inputs: list[int],
        strategies: Optional[list] = None,
        label: str = "",
    ) -> int:
        """
        Poseidon hash with multiple implementation strategies.
        """
        from dcl_poc.ir.nodes import poseidon_strategies

        nid = self._alloc_id()
        strats = strategies or poseidon_strategies(len(inputs))
        node = DCIRNode(
            id=nid, node_type=NodeType.POSEIDON,
            inputs=inputs, strategies=strats,
            label=label or f"poseidon_{nid}",
        )
        self.nodes[nid] = node
        return nid

    def assert_eq(self, a: int, b: int, label: str = "") -> int:
        """Equality constraint: a == b."""
        nid = self._alloc_id()
        node = DCIRNode(
            id=nid, node_type=NodeType.ASSERT_EQ,
            inputs=[a, b], label=label or f"eq_{nid}",
        )
        self.nodes[nid] = node
        return nid

    def assert_bool(self, x: int, label: str = "") -> int:
        """Boolean constraint: x * (1 - x) == 0."""
        nid = self._alloc_id()
        node = DCIRNode(
            id=nid, node_type=NodeType.ASSERT_BOOL,
            inputs=[x], label=label or f"bool_{nid}",
        )
        self.nodes[nid] = node
        return nid

    def select(self, cond: int, if_true: int, if_false: int, label: str = "") -> int:
        """MUX gate: output = cond ? if_true : if_false. Costs 1 mul."""
        nid = self._alloc_id()
        node = DCIRNode(
            id=nid, node_type=NodeType.SELECT,
            inputs=[cond, if_true, if_false],
            label=label or f"select_{nid}",
        )
        self.nodes[nid] = node
        return nid

    # ============================================================
    # Graph analysis
    # ============================================================

    def topological_sort(self) -> list[int]:
        """Return node IDs in topological order (dependencies first)."""
        visited: set[int] = set()
        order: list[int] = []

        def visit(nid: int):
            if nid in visited:
                return
            visited.add(nid)
            node = self.nodes[nid]
            for inp in node.inputs:
                visit(inp)
            order.append(nid)

        for nid in self.nodes:
            visit(nid)

        return order

    def get_optimizable_nodes(self) -> list[DCIRNode]:
        """Return all nodes that have multiple implementation strategies."""
        return [n for n in self.nodes.values() if n.is_optimizable]

    def count_muls_fixed(self) -> int:
        """
        Count the total number of multiplication constraints assuming
        a fixed (default) strategy for each optimizable node.

        This serves as the baseline for comparison.
        """
        total = 0
        for node in self.nodes.values():
            if node.node_type == NodeType.MUL:
                total += 1
            elif node.node_type == NodeType.SELECT:
                total += 1  # MUX costs 1 mul
            elif node.node_type == NodeType.ASSERT_BOOL:
                total += 1  # b*(1-b)=0 costs 1 mul
            elif node.is_optimizable and node.strategies:
                # Default: pick the first strategy (unoptimized)
                total += int(node.strategies[0].constraint_cost)
        return total

    def depth(self) -> int:
        """Compute the maximum depth (longest path) of the DAG."""
        depths: dict[int, int] = {}

        for nid in self.topological_sort():
            node = self.nodes[nid]
            if not node.inputs:
                depths[nid] = 0
            else:
                depths[nid] = max(depths.get(inp, 0) for inp in node.inputs) + 1

        return max(depths.values()) if depths else 0

    def summary(self) -> dict:
        """Return a summary of the graph."""
        type_counts: dict[str, int] = defaultdict(int)
        for node in self.nodes.values():
            type_counts[node.node_type.value] += 1

        optimizable = self.get_optimizable_nodes()
        return {
            "name": self.name,
            "total_nodes": len(self.nodes),
            "node_types": dict(type_counts),
            "optimizable_nodes": len(optimizable),
            "baseline_constraints": self.count_muls_fixed(),
            "depth": self.depth(),
        }

    def __repr__(self) -> str:
        s = self.summary()
        return (
            f"DCIRGraph('{s['name']}', "
            f"nodes={s['total_nodes']}, "
            f"optimizable={s['optimizable_nodes']}, "
            f"baseline_constraints={s['baseline_constraints']}, "
            f"depth={s['depth']})"
        )

"""
DCIR Node definitions.

Each node in the Differentiable Cryptographic IR represents a single
arithmetic operation, constraint, or structural decision point.

Key innovation: nodes carry *learnable relaxation parameters* (alpha)
that control which implementation strategy the compiler selects.
These parameters are optimized via gradient descent during compilation.
"""

from __future__ import annotations

import enum
from dataclasses import dataclass, field
from typing import Optional


class NodeType(enum.Enum):
    """Types of operations in DCIR."""
    # --- Arithmetic primitives ---
    CONST = "const"         # Constant field element
    INPUT = "input"         # Circuit input (public or private)
    ADD = "add"             # Addition (FREE in R1CS — no constraint)
    SUB = "sub"             # Subtraction (FREE in R1CS)
    MUL = "mul"             # Multiplication (COSTS 1 R1CS constraint)
    DIV = "div"             # Division (implemented as constrained inverse mul)

    # --- Control flow (flattened) ---
    SELECT = "select"       # if-then-else → MUX gate (costs 1 mul)

    # --- Constraints ---
    ASSERT_EQ = "assert_eq"       # Equality constraint
    ASSERT_BOOL = "assert_bool"   # Boolean constraint: b*(1-b)=0
    RANGE_CHECK = "range_check"   # Range check: x < 2^n

    # --- Cryptographic primitives ---
    POSEIDON = "poseidon"         # Poseidon hash (ZK-friendly)
    PEDERSEN = "pedersen"         # Pedersen commitment

    # --- Structural ---
    SUBCIRCUIT = "subcircuit"     # Sub-circuit call


class InputVisibility(enum.Enum):
    """Visibility of a circuit input."""
    PUBLIC = "public"
    PRIVATE = "private"


@dataclass
class ImplementationStrategy:
    """
    One possible implementation strategy for a node.

    For example, a RANGE_CHECK node (proving x < 2^8) might have:
      - Strategy A: boolean decomposition (8 constraints, depth 1)
      - Strategy B: lookup table        (1 constraint,  depth 3)
      - Strategy C: polynomial approx   (3 constraints, depth 3)
    """
    name: str
    constraint_cost: float    # Number of R1CS multiplication constraints
    depth_cost: float         # Circuit depth contribution
    noise_cost: float         # FHE noise growth factor (for FHE backend)

    def __repr__(self) -> str:
        return (
            f"Strategy({self.name}: constraints={self.constraint_cost}, "
            f"depth={self.depth_cost}, noise={self.noise_cost})"
        )


@dataclass
class DCIRNode:
    """
    A single node in the DCIR computation graph.

    Attributes:
        id: Unique node identifier.
        node_type: The type of operation.
        inputs: List of input node IDs.
        strategies: Available implementation strategies (for optimizable nodes).
        alpha: Learnable logits for strategy selection (Gumbel-Softmax).
        value: Optional constant value (for CONST nodes).
        bits: Bit width (for RANGE_CHECK nodes).
        visibility: Input visibility (for INPUT nodes).
        label: Human-readable label for debugging.
    """
    id: int
    node_type: NodeType
    inputs: list[int] = field(default_factory=list)

    # --- Implementation strategies (the differentiable part) ---
    strategies: list[ImplementationStrategy] = field(default_factory=list)
    # Alpha logits: one per strategy, optimized via gradient descent
    # Initialized to uniform (equal preference for all strategies)
    alpha: Optional[list[float]] = None

    # --- Node-specific attributes ---
    value: Optional[float] = None            # For CONST
    bits: Optional[int] = None               # For RANGE_CHECK
    visibility: Optional[InputVisibility] = None  # For INPUT
    label: str = ""

    def __post_init__(self):
        if self.strategies and self.alpha is None:
            # Initialize alpha to uniform logits
            n = len(self.strategies)
            self.alpha = [0.0] * n  # uniform in logit space

    @property
    def is_optimizable(self) -> bool:
        """Whether this node has multiple implementation strategies."""
        return len(self.strategies) > 1

    @property
    def is_linear(self) -> bool:
        """Whether this operation is 'free' in R1CS (no multiplication)."""
        return self.node_type in (NodeType.ADD, NodeType.SUB, NodeType.CONST, NodeType.INPUT)

    def __repr__(self) -> str:
        parts = [f"Node({self.id}, {self.node_type.value}"]
        if self.inputs:
            parts.append(f", inputs={self.inputs}")
        if self.label:
            parts.append(f", label='{self.label}'")
        if self.is_optimizable:
            strat_names = [s.name for s in self.strategies]
            parts.append(f", strategies={strat_names}")
        parts.append(")")
        return "".join(parts)


# ============================================================
# Pre-defined implementation strategies for common operations
# ============================================================

def range_check_strategies(bits: int) -> list[ImplementationStrategy]:
    """
    Generate implementation strategies for a range check proving x < 2^bits.

    Strategy A (Boolean Decomposition):
        Decompose x into individual bits, each constrained to be boolean.
        Cost: `bits` constraints (one b*(1-b)=0 per bit + reconstruction).
        Depth: 1 (all constraints are parallel).

    Strategy B (Lookup Table):
        Use a pre-computed lookup table to verify the value.
        Cost: ~1 constraint (table membership proof).
        Depth: log2(table_size) — depends on table structure.
        Note: Higher setup cost, better for repeated checks.

    Strategy C (Polynomial Approximation):
        Use a polynomial identity to bound the value.
        Cost: ~ceil(bits/2) constraints.
        Depth: ceil(bits/2) — sequential polynomial evaluation.
    """
    return [
        ImplementationStrategy(
            name="boolean_decomp",
            constraint_cost=float(bits),  # 1 constraint per bit
            depth_cost=1.0,               # All parallel
            noise_cost=float(bits) * 0.5, # Low noise per bit
        ),
        ImplementationStrategy(
            name="lookup_table",
            constraint_cost=1.0,                  # Single lookup
            depth_cost=max(1.0, float(bits) / 2),  # Log depth
            noise_cost=float(bits) * 1.5,          # Higher noise for lookup
        ),
        ImplementationStrategy(
            name="polynomial_approx",
            constraint_cost=max(1.0, float(bits) / 2),  # ~bits/2
            depth_cost=max(1.0, float(bits) / 2),        # Sequential
            noise_cost=float(bits) * 2.0,                # Highest noise
        ),
    ]


def poseidon_strategies(num_inputs: int) -> list[ImplementationStrategy]:
    """
    Generate implementation strategies for Poseidon hash.

    Strategy A (Full Rounds):
        Standard full-round Poseidon with all R_F + R_P rounds.
        Highest security, highest constraint count.

    Strategy B (Partial Rounds Optimized):
        Use algebraic optimizations to reduce partial round constraints.
        Slightly lower constraint count.

    Strategy C (Lookup-Assisted):
        Replace S-box computations with lookup arguments.
        Lowest constraints but requires lookup support from backend.
    """
    # Poseidon cost model: based on t=num_inputs+1 state width
    t = num_inputs + 1
    base_full = 8   # R_F = 8 full rounds
    base_partial = max(56, 3 * t)  # R_P partial rounds (security-dependent)

    # Each round with x^5 S-box costs 1 mul per state element (full) or 1 total (partial)
    full_round_cost = t  # All state elements get S-box
    partial_round_cost = 1  # Only 1 state element gets S-box

    return [
        ImplementationStrategy(
            name="standard",
            constraint_cost=float(base_full * full_round_cost + base_partial * partial_round_cost),
            depth_cost=float(base_full + base_partial),
            noise_cost=float(base_full * full_round_cost + base_partial) * 0.8,
        ),
        ImplementationStrategy(
            name="partial_optimized",
            constraint_cost=float(base_full * full_round_cost + base_partial * 0.6),
            depth_cost=float(base_full + base_partial * 0.7),
            noise_cost=float(base_full * full_round_cost + base_partial * 0.6) * 0.9,
        ),
        ImplementationStrategy(
            name="lookup_assisted",
            constraint_cost=float(base_full * 2 + base_partial * 0.3),
            depth_cost=float(base_full + base_partial * 0.5),
            noise_cost=float(base_full * 3 + base_partial) * 1.2,
        ),
    ]


def merkle_hash_strategies() -> list[ImplementationStrategy]:
    """
    Strategies for the hash function used inside a Merkle tree node.

    Strategy A: Poseidon merge (standard)
    Strategy B: Poseidon merge (optimized partial rounds)
    Strategy C: Algebraic hash (lower cost, different security assumptions)
    """
    return [
        ImplementationStrategy(
            name="poseidon_standard",
            constraint_cost=320.0,
            depth_cost=64.0,
            noise_cost=250.0,
        ),
        ImplementationStrategy(
            name="poseidon_optimized",
            constraint_cost=220.0,
            depth_cost=50.0,
            noise_cost=280.0,
        ),
        ImplementationStrategy(
            name="algebraic_hash",
            constraint_cost=150.0,
            depth_cost=40.0,
            noise_cost=180.0,
        ),
    ]

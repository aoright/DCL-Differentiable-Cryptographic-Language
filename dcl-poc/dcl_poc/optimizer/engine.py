"""
Differentiable Optimization Engine.

This is the core of DCL's compiler innovation: it replaces traditional
heuristic compiler passes with an end-to-end differentiable optimization
loop that uses gradient descent to find the optimal strategy assignment
for each node in the DCIR graph.

Algorithm:
  1. Extract all optimizable nodes and their strategy cost matrices
  2. Initialize alpha logits (strategy preferences) to uniform
  3. FOR each epoch:
     a. Compute total loss L_total (differentiable w.r.t. alphas)
     b. Compute gradients ∂L/∂α via JAX autodiff
     c. Update alphas with Adam optimizer
     d. Anneal temperature τ
  4. Discretize: argmax each alpha → final strategy selection
"""

from __future__ import annotations

import time
from dataclasses import dataclass, field
from typing import Optional

import jax
import jax.numpy as jnp
import numpy as np

from dcl_poc.ir.graph import DCIRGraph
from dcl_poc.ir.nodes import DCIRNode, NodeType
from dcl_poc.ir.cost_model import get_strategy_cost_matrix, get_fixed_cost
from dcl_poc.optimizer.loss import total_loss
from dcl_poc.optimizer.gumbel import (
    annealing_schedule,
    get_selected_strategy,
    softmax_no_noise,
)


@dataclass
class OptimizationConfig:
    """Configuration for the differentiable optimization engine."""
    max_epochs: int = 300
    learning_rate: float = 0.05
    tau_start: float = 5.0
    tau_end: float = 0.05
    w_constraints: float = 1.0
    w_depth: float = 0.1
    w_noise: float = 0.05
    w_entropy: float = 0.05
    # Adam optimizer hyperparameters
    adam_beta1: float = 0.9
    adam_beta2: float = 0.999
    adam_eps: float = 1e-8
    # Logging
    log_interval: int = 50
    verbose: bool = True


@dataclass
class OptimizationResult:
    """Result of the differentiable optimization."""
    # Strategy selections: node_id → selected strategy index
    selections: dict[int, int] = field(default_factory=dict)
    # Strategy names: node_id → selected strategy name
    selection_names: dict[int, str] = field(default_factory=dict)
    # Optimized total constraint count
    optimized_constraints: float = 0.0
    # Baseline (unoptimized) constraint count
    baseline_constraints: float = 0.0
    # Reduction ratio
    reduction_pct: float = 0.0
    # Training history
    loss_history: list[float] = field(default_factory=list)
    tau_history: list[float] = field(default_factory=list)
    # Timing
    elapsed_seconds: float = 0.0


class DifferentiableOptimizer:
    """
    The differentiable optimization engine.

    Takes a DCIR graph and optimizes all strategy-selection parameters
    (alpha logits) via gradient descent to minimize the total circuit cost.
    """

    def __init__(self, config: Optional[OptimizationConfig] = None):
        self.config = config or OptimizationConfig()

    def optimize(self, graph: DCIRGraph) -> OptimizationResult:
        """
        Run differentiable optimization on the given DCIR graph.

        Returns an OptimizationResult with the optimal strategy selections.
        """
        cfg = self.config
        t_start = time.time()

        # ── Step 1: Extract optimizable nodes and cost matrices ──
        opt_nodes: list[DCIRNode] = graph.get_optimizable_nodes()

        if not opt_nodes:
            if cfg.verbose:
                print("  No optimizable nodes found. Nothing to optimize.")
            baseline = graph.count_muls_fixed()
            return OptimizationResult(
                baseline_constraints=baseline,
                optimized_constraints=baseline,
                reduction_pct=0.0,
            )

        # Build cost matrices as JAX arrays
        cost_matrices: list[jnp.ndarray] = []
        for node in opt_nodes:
            cm = get_strategy_cost_matrix(node)
            cost_matrices.append(jnp.array(cm, dtype=jnp.float32))

        # Build fixed costs for non-optimizable nodes
        fixed_cost_list = []
        for node in graph.nodes.values():
            if not node.is_optimizable:
                fc = get_fixed_cost(node)
                fixed_cost_list.append(fc)
        fixed_costs = jnp.array(fixed_cost_list, dtype=jnp.float32) if fixed_cost_list else jnp.zeros((0, 3))

        # ── Step 2: Initialize alpha logits ──
        # Start with uniform logits (no preference)
        alpha_params = [jnp.zeros(len(node.strategies), dtype=jnp.float32) for node in opt_nodes]

        # ── Step 3: Adam optimizer state ──
        m_states = [jnp.zeros_like(a) for a in alpha_params]  # First moment
        v_states = [jnp.zeros_like(a) for a in alpha_params]  # Second moment

        # ── Step 4: Optimization loop ──
        loss_history = []
        tau_history = []
        baseline = graph.count_muls_fixed()

        if cfg.verbose:
            print(f"  Optimizing {len(opt_nodes)} nodes across {cfg.max_epochs} epochs")
            print(f"  Baseline constraints: {baseline}")
            print(f"  {'Epoch':>6} | {'Loss':>10} | {'τ':>6} | {'Est. Constraints':>16}")
            print(f"  {'-'*6} | {'-'*10} | {'-'*6} | {'-'*16}")

        for epoch in range(cfg.max_epochs):
            tau = annealing_schedule(epoch, cfg.max_epochs, cfg.tau_start, cfg.tau_end)
            tau_history.append(tau)

            # --- Forward + Backward (JAX autodiff) ---
            def loss_fn(alphas_flat):
                # Reconstruct list of alpha arrays from flat vector
                alphas = []
                offset = 0
                for node in opt_nodes:
                    n_strats = len(node.strategies)
                    alphas.append(alphas_flat[offset:offset + n_strats])
                    offset += n_strats

                return total_loss(
                    all_alpha_logits=alphas,
                    all_cost_matrices=cost_matrices,
                    fixed_costs=fixed_costs,
                    tau=tau,
                    w_constraints=cfg.w_constraints,
                    w_depth=cfg.w_depth,
                    w_noise=cfg.w_noise,
                    w_entropy=cfg.w_entropy,
                )

            # Flatten params for JAX
            alphas_flat = jnp.concatenate(alpha_params)

            # Compute loss and gradient
            loss_val, grad_flat = jax.value_and_grad(loss_fn)(alphas_flat)
            loss_history.append(float(loss_val))

            # --- Adam update ---
            offset = 0
            new_alpha_params = []
            new_m_states = []
            new_v_states = []

            for i, node in enumerate(opt_nodes):
                n_strats = len(node.strategies)
                g = grad_flat[offset:offset + n_strats]

                # Adam moments
                m = cfg.adam_beta1 * m_states[i] + (1 - cfg.adam_beta1) * g
                v = cfg.adam_beta2 * v_states[i] + (1 - cfg.adam_beta2) * g ** 2

                # Bias correction
                m_hat = m / (1 - cfg.adam_beta1 ** (epoch + 1))
                v_hat = v / (1 - cfg.adam_beta2 ** (epoch + 1))

                # Parameter update
                a = alpha_params[i] - cfg.learning_rate * m_hat / (jnp.sqrt(v_hat) + cfg.adam_eps)

                new_alpha_params.append(a)
                new_m_states.append(m)
                new_v_states.append(v)
                offset += n_strats

            alpha_params = new_alpha_params
            m_states = new_m_states
            v_states = new_v_states

            # --- Logging ---
            if cfg.verbose and (epoch % cfg.log_interval == 0 or epoch == cfg.max_epochs - 1):
                # Estimate current constraint count
                est_constraints = self._estimate_constraints(alpha_params, opt_nodes, cost_matrices, tau, fixed_costs)
                print(f"  {epoch:>6} | {float(loss_val):>10.2f} | {tau:>6.3f} | {est_constraints:>16.1f}")

        # ── Step 5: Discretize — argmax each alpha ──
        selections = {}
        selection_names = {}
        optimized_constraints = 0.0

        for i, node in enumerate(opt_nodes):
            idx = get_selected_strategy(alpha_params[i])
            selections[node.id] = idx
            selection_names[node.id] = node.strategies[idx].name
            optimized_constraints += node.strategies[idx].constraint_cost

        # Add fixed costs
        for node in graph.nodes.values():
            if not node.is_optimizable:
                fc = get_fixed_cost(node)
                optimized_constraints += fc[0]

        reduction = (1 - optimized_constraints / baseline) * 100 if baseline > 0 else 0

        elapsed = time.time() - t_start

        if cfg.verbose:
            print(f"\n  ✅ Optimization complete in {elapsed:.2f}s")
            print(f"  Baseline:  {baseline} constraints")
            print(f"  Optimized: {optimized_constraints:.0f} constraints")
            print(f"  Reduction: {reduction:.1f}%")
            print(f"  Selections:")
            for nid, name in selection_names.items():
                node = graph.nodes[nid]
                print(f"    {node.label}: {name}")

        return OptimizationResult(
            selections=selections,
            selection_names=selection_names,
            optimized_constraints=optimized_constraints,
            baseline_constraints=baseline,
            reduction_pct=reduction,
            loss_history=loss_history,
            tau_history=tau_history,
            elapsed_seconds=elapsed,
        )

    def _estimate_constraints(
        self,
        alpha_params: list[jnp.ndarray],
        opt_nodes: list[DCIRNode],
        cost_matrices: list[jnp.ndarray],
        tau: float,
        fixed_costs: jnp.ndarray,
    ) -> float:
        """Estimate current constraint count using soft strategy probabilities."""
        total = 0.0
        for i, node in enumerate(opt_nodes):
            probs = softmax_no_noise(alpha_params[i], tau)
            constraints = cost_matrices[i][:, 0]  # constraint column
            total += float(jnp.dot(probs, constraints))

        # Add fixed
        if fixed_costs.shape[0] > 0:
            total += float(jnp.sum(fixed_costs[:, 0]))

        return total

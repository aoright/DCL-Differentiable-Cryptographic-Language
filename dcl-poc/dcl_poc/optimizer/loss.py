"""
Differentiable loss function for DCIR circuit optimization.

The total loss L_total combines multiple objectives:

  L_total = w₁·L_constraints + w₂·L_depth + w₃·L_regularization

Where:
  - L_constraints: Total number of R1CS multiplication constraints
    (weighted by Gumbel-Softmax strategy probabilities)
  - L_depth: Maximum circuit depth (affects verification time)
  - L_regularization: Entropy bonus to encourage exploration early on

All components are differentiable w.r.t. the strategy logits (alpha).
"""

from __future__ import annotations

import jax
import jax.numpy as jnp

from dcl_poc.optimizer.gumbel import softmax_no_noise


def compute_weighted_cost(
    alpha_logits: jnp.ndarray,
    cost_matrix: jnp.ndarray,
    tau: float = 1.0,
) -> jnp.ndarray:
    """
    Compute the soft-weighted cost for a single optimizable node.

    Args:
        alpha_logits: Strategy logits, shape (num_strategies,)
        cost_matrix: Cost vectors per strategy, shape (num_strategies, 3)
                     where columns are [constraints, depth, noise]
        tau: Gumbel-Softmax temperature

    Returns:
        Weighted cost vector, shape (3,) — [constraints, depth, noise]
    """
    # Soft strategy selection probabilities
    probs = softmax_no_noise(alpha_logits, tau)  # shape (num_strategies,)
    # Weighted sum of costs
    weighted = jnp.einsum("s,sd->d", probs, cost_matrix)
    return weighted


def loss_constraints(weighted_costs: jnp.ndarray) -> jnp.ndarray:
    """
    L_constraints: sum of all constraint costs across all nodes.

    Args:
        weighted_costs: shape (num_nodes, 3) — [constraints, depth, noise] per node
    Returns:
        Scalar loss
    """
    return jnp.sum(weighted_costs[:, 0])


def loss_depth(weighted_costs: jnp.ndarray) -> jnp.ndarray:
    """
    L_depth: maximum depth across all nodes.

    We use a smooth approximation of max via log-sum-exp.
    """
    depths = weighted_costs[:, 1]
    # Smooth max: log(sum(exp(x))) ≈ max(x) for large values
    return jax.nn.logsumexp(depths)


def loss_noise(weighted_costs: jnp.ndarray) -> jnp.ndarray:
    """
    L_noise: total noise budget consumption.
    Relevant primarily for FHE backends.
    """
    return jnp.sum(weighted_costs[:, 2])


def entropy_bonus(all_logits: list[jnp.ndarray], tau: float) -> jnp.ndarray:
    """
    Entropy regularization to encourage exploration early in optimization.

    Higher entropy = more uniform strategy probabilities = more exploration.
    As tau decreases, this term naturally diminishes.
    """
    total_entropy = jnp.float32(0.0)
    for logits in all_logits:
        probs = softmax_no_noise(logits, tau)
        # H = -sum(p * log(p))
        entropy = -jnp.sum(probs * jnp.log(probs + 1e-8))
        total_entropy = total_entropy + entropy
    return total_entropy


def total_loss(
    all_alpha_logits: list[jnp.ndarray],
    all_cost_matrices: list[jnp.ndarray],
    fixed_costs: jnp.ndarray,
    tau: float,
    w_constraints: float = 1.0,
    w_depth: float = 0.1,
    w_noise: float = 0.05,
    w_entropy: float = 0.01,
) -> jnp.ndarray:
    """
    Compute the total differentiable loss function.

    Args:
        all_alpha_logits: List of logit arrays, one per optimizable node.
        all_cost_matrices: List of cost matrices, one per optimizable node.
            Each has shape (num_strategies, 3).
        fixed_costs: Costs from non-optimizable nodes, shape (num_fixed, 3).
        tau: Current temperature for Gumbel-Softmax.
        w_constraints: Weight for constraint count loss.
        w_depth: Weight for depth loss.
        w_noise: Weight for noise loss.
        w_entropy: Weight for entropy bonus (negative = encourages exploration).

    Returns:
        Scalar total loss.
    """
    # Compute soft-weighted costs for all optimizable nodes
    opt_costs = []
    for logits, costs in zip(all_alpha_logits, all_cost_matrices):
        wc = compute_weighted_cost(logits, costs, tau)
        opt_costs.append(wc)

    if opt_costs:
        opt_costs_array = jnp.stack(opt_costs)  # (num_opt_nodes, 3)
    else:
        opt_costs_array = jnp.zeros((0, 3))

    # Combine optimizable and fixed costs
    if fixed_costs.shape[0] > 0:
        all_costs = jnp.concatenate([opt_costs_array, fixed_costs], axis=0)
    else:
        all_costs = opt_costs_array

    # Individual loss components
    l_constraints = loss_constraints(all_costs)
    l_depth = loss_depth(all_costs)
    l_noise = loss_noise(all_costs)
    l_entropy = entropy_bonus(all_alpha_logits, tau) if all_alpha_logits else jnp.float32(0.0)

    # Total weighted loss
    loss = (
        w_constraints * l_constraints
        + w_depth * l_depth
        + w_noise * l_noise
        - w_entropy * l_entropy  # Negative because we MAXIMIZE entropy
    )

    return loss

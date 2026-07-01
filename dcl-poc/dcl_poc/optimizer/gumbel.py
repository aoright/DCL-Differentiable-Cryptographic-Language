"""
Gumbel-Softmax implementation for differentiable discrete optimization.

The Gumbel-Softmax trick (Jang et al., 2017) allows us to sample from a
categorical distribution in a differentiable way. This is the key technique
that makes our compiler's strategy selection differentiable.

During optimization:
  - High temperature (τ >> 0): soft selection, gradient flows to all strategies
  - Low temperature (τ → 0):  hard selection, converges to argmax

We use an annealing schedule to gradually decrease τ during the optimization loop.
"""

from __future__ import annotations

import jax
import jax.numpy as jnp


def gumbel_softmax(
    logits: jnp.ndarray,
    tau: float = 1.0,
    key: jax.random.PRNGKey = None,
    hard: bool = False,
) -> jnp.ndarray:
    """
    Sample from the Gumbel-Softmax distribution.

    Args:
        logits: Unnormalized log-probabilities, shape (num_strategies,)
        tau: Temperature parameter. Lower = more discrete.
        key: JAX PRNG key for sampling Gumbel noise.
        hard: If True, use straight-through estimator for hard samples.

    Returns:
        Soft (or hard) categorical sample, shape (num_strategies,)
    """
    if key is None:
        key = jax.random.PRNGKey(0)

    # Sample Gumbel noise: g = -log(-log(u)), u ~ Uniform(0, 1)
    u = jax.random.uniform(key, shape=logits.shape, minval=1e-8, maxval=1.0)
    gumbel_noise = -jnp.log(-jnp.log(u))

    # Apply Gumbel-Softmax
    y_soft = jax.nn.softmax((logits + gumbel_noise) / tau)

    if hard:
        # Straight-Through Estimator: forward uses argmax, backward uses soft
        y_hard = jnp.zeros_like(y_soft)
        y_hard = y_hard.at[jnp.argmax(y_soft)].set(1.0)
        # Stop gradient on the difference so backward pass uses y_soft gradients
        y = y_hard - jax.lax.stop_gradient(y_soft) + y_soft
        return y

    return y_soft


def softmax_no_noise(logits: jnp.ndarray, tau: float = 1.0) -> jnp.ndarray:
    """
    Deterministic softmax with temperature (no Gumbel noise).
    Used for evaluation / final cost computation.
    """
    return jax.nn.softmax(logits / tau)


def annealing_schedule(
    epoch: int,
    max_epochs: int,
    tau_start: float = 5.0,
    tau_end: float = 0.1,
) -> float:
    """
    Exponential annealing schedule for the temperature parameter.

    Starts at tau_start (very soft/exploratory) and decays to tau_end
    (nearly discrete) over max_epochs.
    """
    decay_rate = (tau_end / tau_start) ** (1.0 / max(1, max_epochs))
    return max(tau_end, tau_start * (decay_rate ** epoch))


def get_selected_strategy(logits: jnp.ndarray) -> int:
    """
    After optimization, discretize by taking argmax.
    Returns the index of the selected strategy.
    """
    return int(jnp.argmax(logits))

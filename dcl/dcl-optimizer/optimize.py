#!/usr/bin/env python3
"""
DCL Differentiable Strategy Optimizer.

Uses JAX with Gumbel-Softmax continuous relaxation and gradient descent to select
optimal implementation strategies for each DCIR node. Strategies are parameterized
by continuous alpha vectors which are annealed to discrete selections via temperature
scheduling.

Features:
- Gumbel-Softmax with temperature annealing (τ: 5.0 → 0.05)
- Cosine learning rate schedule
- Early stopping on convergence
- Topology-aware depth cost model
- Multi-objective loss: constraints + depth + noise + entropy regularization
"""

import argparse
import json
import sys
import math
import os

# Attempt JAX import with graceful fallback
try:
    os.environ.setdefault("JAX_PLATFORM_NAME", "cpu")
    import jax
    import jax.numpy as jnp
    from jax import grad
    HAS_JAX = True
except ImportError:
    HAS_JAX = False
    print("⚠️  JAX not installed. Using numpy fallback (no gradient optimization).")
    import numpy as jnp
    import numpy


def load_graph(path: str) -> dict:
    """Load a DCIR graph from a JSON file."""
    with open(path, 'r') as f:
        return json.load(f)


def save_graph(graph: dict, path: str):
    """Save a DCIR graph to a JSON file."""
    with open(path, 'w') as f:
        json.dump(graph, f, indent=2)


def compute_topology_depth(graph: dict) -> dict:
    """Compute topological depth for each node in the DAG."""
    node_map = {n['id']: n for n in graph['nodes']}
    depth = {}

    def get_depth(nid):
        if nid in depth:
            return depth[nid]
        node = node_map.get(nid)
        if node is None or not node['inputs']:
            depth[nid] = 0
            return 0
        d = 1 + max(get_depth(inp) for inp in node['inputs'])
        depth[nid] = d
        return d

    for n in graph['nodes']:
        get_depth(n['id'])

    return depth


def gumbel_softmax(alpha, tau, key=None):
    """
    Gumbel-Softmax with stochastic noise for exploration.

    When key is provided, adds Gumbel noise for exploration during optimization.
    Without key, returns deterministic softmax (used at final discretization).
    """
    if key is not None and HAS_JAX:
        gumbel_noise = jax.random.gumbel(key, shape=alpha.shape) * 0.1
        logits = (alpha + gumbel_noise) / tau
    else:
        logits = alpha / tau

    # Numerically stable softmax via logsumexp
    max_logit = jnp.max(logits, axis=-1, keepdims=True)
    exp_logits = jnp.exp(logits - max_logit)
    return exp_logits / jnp.sum(exp_logits, axis=-1, keepdims=True)


def cosine_lr(base_lr: float, epoch: int, total_epochs: int, min_lr: float = 1e-4) -> float:
    """Cosine annealing learning rate schedule."""
    return min_lr + 0.5 * (base_lr - min_lr) * (1 + math.cos(math.pi * epoch / total_epochs))


def build_loss_fn(cost_matrices, topo_depths, w_constraints=1.0, w_depth=0.5, w_noise=0.3, w_entropy=0.1):
    """
    Build the multi-objective differentiable loss function.

    Args:
        cost_matrices: shape (num_nodes, num_strategies, 3) — [constraint, depth, noise]
        topo_depths: per-node topological depth weights
        w_constraints, w_depth, w_noise: objective weights
        w_entropy: entropy regularization weight (prevents premature collapse)
    """
    def loss_fn(alphas, tau, keys=None):
        total_loss = 0.0
        num_nodes = len(cost_matrices)

        for i in range(num_nodes):
            costs_i = cost_matrices[i]     # (num_strategies, 3)
            alpha_i = alphas[i]            # (num_strategies,)
            key_i = keys[i] if keys is not None else None

            probs = gumbel_softmax(alpha_i, tau, key_i)

            # Weighted cost components
            constraint_cost = jnp.sum(probs * costs_i[:, 0])
            depth_cost = jnp.sum(probs * costs_i[:, 1])
            noise_cost = jnp.sum(probs * costs_i[:, 2])

            # Topology-aware depth weighting: deeper nodes contribute more to critical path
            depth_weight = 1.0 + topo_depths[i] * 0.1

            # Entropy regularization: H(p) = -sum(p * log(p))
            entropy = -jnp.sum(probs * jnp.log(probs + 1e-10))

            node_loss = (
                w_constraints * constraint_cost +
                w_depth * depth_cost * depth_weight +
                w_noise * noise_cost -
                w_entropy * entropy
            )
            total_loss = total_loss + node_loss

        return total_loss / max(num_nodes, 1)

    return loss_fn


def optimize(graph: dict, epochs: int = 300, base_lr: float = 0.05) -> dict:
    """
    Run differentiable strategy optimization on the DCIR graph.

    Returns the graph with optimal alpha vectors assigned to each node.
    """
    # Collect nodes with strategies
    strategy_nodes = []
    cost_matrices = []
    topo_depths_map = compute_topology_depth(graph)

    for node in graph['nodes']:
        if node.get('strategies') and len(node['strategies']) > 0:
            strategy_nodes.append(node)
            costs = []
            for s in node['strategies']:
                costs.append([
                    s.get('constraint_cost', 1.0),
                    s.get('depth_cost', 1.0),
                    s.get('noise_cost', 1.0)
                ])
            cost_matrices.append(jnp.array(costs))

    if not strategy_nodes:
        print("   No strategy-annotated nodes found. Skipping optimization.")
        return graph

    num_nodes = len(strategy_nodes)
    topo_depths = jnp.array([topo_depths_map.get(n['id'], 0) for n in strategy_nodes], dtype=jnp.float32)

    # Initialize alphas (all zeros = uniform prior)
    alphas = [jnp.zeros(len(cm)) for cm in cost_matrices]

    loss_fn = build_loss_fn(cost_matrices, topo_depths)

    if HAS_JAX:
        key = jax.random.PRNGKey(42)
        grad_fn = grad(lambda a, t, k: loss_fn(a, t, k))
    else:
        key = None
        grad_fn = None

    # Temperature schedule: exponential decay from 5.0 to 0.05
    tau_start = 5.0
    tau_end = 0.05

    best_loss = float('inf')
    patience = 0
    patience_limit = 30  # early stop after 30 epochs without improvement
    min_improvement = 1e-6

    print(f"   🔧 Optimizing {num_nodes} strategy nodes over {epochs} epochs...")

    for epoch in range(epochs):
        # Temperature annealing
        progress = epoch / max(epochs - 1, 1)
        tau = tau_start * (tau_end / tau_start) ** progress

        # Learning rate schedule
        lr = cosine_lr(base_lr, epoch, epochs)

        # Generate JAX random keys if JAX is available
        if HAS_JAX:
            key, subkey = jax.random.split(key)
            subkeys = jax.random.split(subkey, num_nodes)
        else:
            subkeys = None

        # Compute loss
        current_loss = float(loss_fn(alphas, tau, subkeys))

        # Early stopping check
        if current_loss < best_loss - min_improvement:
            best_loss = current_loss
            patience = 0
        else:
            patience += 1

        if patience >= patience_limit and epoch > 50:
            print(f"   ⏱️  Early stopping at epoch {epoch} (no improvement for {patience_limit} epochs)")
            break

        # Gradient update (only with JAX)
        if grad_fn is not None:
            grads = grad_fn(alphas, tau, subkeys)
            alphas = [a - lr * g for a, g in zip(alphas, grads)]

        # Progress logging
        if epoch % 50 == 0 or epoch == epochs - 1:
            print(f"   Epoch {epoch:4d} | Loss: {current_loss:.6f} | τ: {tau:.4f} | lr: {lr:.5f}")

    # Discretize: select argmax strategy
    print("\n   📊 Optimization Results:")
    total_constraints = 0
    total_depth = 0
    total_noise = 0

    for i, node in enumerate(strategy_nodes):
        probs = gumbel_softmax(alphas[i], 0.01)  # Very low temp for sharp selection
        selected = int(jnp.argmax(probs))
        node['alpha'] = [float(x) for x in probs]

        strat = node['strategies'][selected]
        total_constraints += strat['constraint_cost']
        total_depth += strat['depth_cost']
        total_noise += strat['noise_cost']
        print(f"      Node '{node['label']}': selected '{strat['name']}' "
              f"(constraints={strat['constraint_cost']:.1f}, depth={strat['depth_cost']:.1f}, noise={strat['noise_cost']:.1f})")

    print(f"\n   📈 Total: constraints={total_constraints:.1f}, depth={total_depth:.1f}, noise={total_noise:.1f}")

    return graph


def main():
    parser = argparse.ArgumentParser(description='DCL Differentiable Strategy Optimizer')
    parser.add_argument('--input', required=True, help='Path to input DCIR graph JSON')
    parser.add_argument('--output', required=True, help='Path to write optimized DCIR graph JSON')
    parser.add_argument('--epochs', type=int, default=300, help='Number of optimization epochs')
    parser.add_argument('--lr', type=float, default=0.05, help='Base learning rate')
    args = parser.parse_args()

    graph = load_graph(args.input)
    optimized = optimize(graph, epochs=args.epochs, base_lr=args.lr)
    save_graph(optimized, args.output)

    print(f"\n   ✅ Optimized graph saved to: {args.output}")


if __name__ == '__main__':
    main()

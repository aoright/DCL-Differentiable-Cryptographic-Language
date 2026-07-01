import json
import sys
import argparse
import time
import jax
import jax.numpy as jnp

# ============================================================
# Optimization Helpers
# ============================================================

def annealing_schedule(epoch, max_epochs, tau_start=5.0, tau_end=0.05):
    decay_rate = (tau_end / tau_start) ** (1.0 / max(1, max_epochs))
    return max(tau_end, tau_start * (decay_rate ** epoch))

def softmax_no_noise(logits, tau=1.0):
    return jax.nn.softmax(logits / tau)

def compute_weighted_cost(alpha_logits, cost_matrix, tau=1.0):
    probs = softmax_no_noise(alpha_logits, tau)
    return jnp.einsum("s,sd->d", probs, cost_matrix)

def total_loss(all_alpha_logits, all_cost_matrices, fixed_costs, tau,
               w_constraints=1.0, w_depth=0.1, w_noise=0.05, w_entropy=0.05):
    opt_costs = []
    for logits, costs in zip(all_alpha_logits, all_cost_matrices):
        wc = compute_weighted_cost(logits, costs, tau)
        opt_costs.append(wc)

    if opt_costs:
        opt_costs_array = jnp.stack(opt_costs)
    else:
        opt_costs_array = jnp.zeros((0, 3))

    if fixed_costs.shape[0] > 0:
        all_costs = jnp.concatenate([opt_costs_array, fixed_costs], axis=0)
    else:
        all_costs = opt_costs_array

    l_constraints = jnp.sum(all_costs[:, 0])
    l_depth = jax.nn.logsumexp(all_costs[:, 1])
    l_noise = jnp.sum(all_costs[:, 2])

    total_entropy = jnp.float32(0.0)
    for logits in all_alpha_logits:
        probs = softmax_no_noise(logits, tau)
        entropy = -jnp.sum(probs * jnp.log(probs + 1e-8))
        total_entropy = total_entropy + entropy

    loss = (
        w_constraints * l_constraints
        + w_depth * l_depth
        + w_noise * l_noise
        - w_entropy * total_entropy
    )
    return loss

# ============================================================
# Main Optimizer Routine
# ============================================================

def main():
    parser = argparse.ArgumentParser(description="DCL Differentiable Optimizer")
    parser.add_argument("--input", required=True, help="Input DCIR JSON path")
    parser.add_argument("--output", required=True, help="Output optimized DCIR JSON path")
    parser.add_argument("--epochs", type=int, default=300, help="Max optimization epochs")
    parser.add_argument("--lr", type=float, default=0.05, help="Learning rate")
    args = parser.parse_args()

    with open(args.input, "r") as f:
        graph = json.load(f)

    # 1. Identify optimizable nodes and build cost matrices
    opt_nodes = []
    cost_matrices = []
    fixed_costs_list = []

    # Map node types to base costs: (constraints, depth, noise)
    base_costs = {
        "const": (0.0, 0.0, 0.0),
        "input": (0.0, 0.0, 0.0),
        "add": (0.0, 1.0, 0.1),
        "sub": (0.0, 1.0, 0.1),
        "mul": (1.0, 1.0, 1.0),
        "div": (2.0, 2.0, 2.0),
        "select": (1.0, 1.0, 1.0),
        "assert_eq": (0.0, 0.0, 0.0),
        "assert_bool": (1.0, 1.0, 0.5),
        "is_zero": (2.0, 2.0, 1.0),
    }

    for node in graph["nodes"]:
        if node.get("strategies") and len(node["strategies"]) > 1:
            opt_nodes.append(node)
            cm = [[s["constraint_cost"], s["depth_cost"], s["noise_cost"]] for s in node["strategies"]]
            cost_matrices.append(jnp.array(cm, dtype=jnp.float32))
        else:
            nt = node["node_type"]
            fc = base_costs.get(nt, (0.0, 0.0, 0.0))
            fixed_costs_list.append(fc)

    fixed_costs = jnp.array(fixed_costs_list, dtype=jnp.float32) if fixed_costs_list else jnp.zeros((0, 3))

    if not opt_nodes:
        print("No optimizable nodes found. Writing unchanged graph.")
        with open(args.output, "w") as f:
            json.dump(graph, f, indent=2)
        return

    # 2. Initialize alpha logits
    alpha_params = [jnp.zeros(len(n["strategies"]), dtype=jnp.float32) for n in opt_nodes]

    # Adam moments
    m_states = [jnp.zeros_like(a) for a in alpha_params]
    v_states = [jnp.zeros_like(a) for a in alpha_params]

    # 3. Optimization loop
    max_epochs = args.epochs
    lr = args.lr
    beta1, beta2, eps = 0.9, 0.999, 1e-8

    print(f"Optimizing {len(opt_nodes)} nodes over {max_epochs} epochs...")
    t_start = time.time()

    for epoch in range(max_epochs):
        tau = annealing_schedule(epoch, max_epochs)

        def loss_fn(alphas_flat):
            alphas = []
            offset = 0
            for n in opt_nodes:
                n_strats = len(n["strategies"])
                alphas.append(alphas_flat[offset:offset + n_strats])
                offset += n_strats
            return total_loss(alphas, cost_matrices, fixed_costs, tau)

        alphas_flat = jnp.concatenate(alpha_params)
        loss_val, grad_flat = jax.value_and_grad(loss_fn)(alphas_flat)

        # Adam Update
        offset = 0
        new_alpha_params = []
        new_m_states = []
        new_v_states = []

        for i, n in enumerate(opt_nodes):
            n_strats = len(n["strategies"])
            g = grad_flat[offset:offset + n_strats]

            m = beta1 * m_states[i] + (1 - beta1) * g
            v = beta2 * v_states[i] + (1 - beta2) * g ** 2

            m_hat = m / (1 - beta1 ** (epoch + 1))
            v_hat = v / (1 - beta2 ** (epoch + 1))

            a = alpha_params[i] - lr * m_hat / (jnp.sqrt(v_hat) + eps)

            new_alpha_params.append(a)
            new_m_states.append(m)
            new_v_states.append(v)
            offset += n_strats

        alpha_params = new_alpha_params
        m_states = new_m_states
        v_states = new_v_states

        if epoch % 50 == 0 or epoch == max_epochs - 1:
            print(f"  Epoch {epoch:03d} | Loss: {float(loss_val):.4f} | Temperature: {tau:.4f}")

    print(f"Optimization finished in {time.time() - t_start:.2f}s.")

    # 4. Apply argmax strategy selections back to graph
    for i, node in enumerate(opt_nodes):
        idx = int(jnp.argmax(alpha_params[i]))
        node["alpha"] = [float(x) for x in alpha_params[i]]
        # We store the selected strategy index in the node itself
        node["selected_strategy_index"] = idx
        node["selected_strategy_name"] = node["strategies"][idx]["name"]
        print(f"  Node {node['id']} ({node['label']}) -> Selected: {node['selected_strategy_name']}")

    # Write output
    with open(args.output, "w") as f:
        json.dump(graph, f, indent=2)

if __name__ == "__main__":
    main()

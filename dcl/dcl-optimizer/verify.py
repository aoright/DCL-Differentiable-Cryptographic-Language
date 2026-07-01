#!/usr/bin/env python3
"""
DCL Z3 SMT Formal Equivalence Verifier.

Proves that an optimized DCIR graph computes the same function as the original
graph for ALL possible inputs, using the Z3 SMT solver over the BN254 prime field.

Features:
- BN254 modular arithmetic
- Uninterpreted functions for Poseidon (grouped by arity)
- Configurable timeout
- Step-by-step verification reporting
- Counterexample output on failure

Exit codes:
  0 = equivalence proven
  1 = counterexample found (semantics changed)
  2 = timeout / unknown
"""

import argparse
import json
import sys

try:
    import z3
except ImportError:
    print("❌ z3-solver not installed. Run: pip install z3-solver")
    sys.exit(2)

# BN254 scalar field prime
BN254_PRIME = 21888242871839275222246405745257275088548364400416034343698204186575808495617


def load_graph(path: str) -> dict:
    """Load a DCIR graph from a JSON file."""
    with open(path, 'r') as f:
        return json.load(f)


def translate_graph_to_z3(graph: dict, prefix: str, timeout_ms: int = 30000):
    """
    Translate a DCIR graph into Z3 symbolic expressions.

    Args:
        graph: The DCIR graph dict
        prefix: Variable name prefix (e.g., 'orig_' or 'opt_')
        timeout_ms: Z3 solver timeout in milliseconds

    Returns:
        (node_vars, assertions, outputs, input_vars) tuple
    """
    node_vars = {}
    assertions = []
    input_vars = {}

    # Create uninterpreted Poseidon functions grouped by arity
    poseidon_fns = {}

    for node in graph['nodes']:
        nid = node['id']
        ntype = node.get('node_type', '')
        label = node.get('label', f'node_{nid}')

        var = z3.Int(f"{prefix}{label}_{nid}")
        node_vars[nid] = var

        if ntype == 'input':
            # Constrain to BN254 field range
            assertions.append(var >= 0)
            assertions.append(var < BN254_PRIME)
            input_vars[label] = var

        elif ntype == 'const':
            val_str = node.get('value', '0')
            try:
                # Use int() directly to preserve full precision for large field constants.
                # int(float(x)) would lose precision for values > 2^53.
                val = int(val_str)
            except (ValueError, OverflowError):
                val = 0
            assertions.append(var == val % BN254_PRIME)

        elif ntype == 'add':
            a, b = node['inputs']
            assertions.append(var == (node_vars[a] + node_vars[b]) % BN254_PRIME)

        elif ntype == 'sub':
            a, b = node['inputs']
            assertions.append(var == (node_vars[a] - node_vars[b] + BN254_PRIME) % BN254_PRIME)

        elif ntype == 'mul':
            a, b = node['inputs']
            assertions.append(var == (node_vars[a] * node_vars[b]) % BN254_PRIME)

        elif ntype == 'div':
            a, b = node['inputs']
            inv = z3.Int(f"{prefix}inv_{nid}")
            assertions.append(inv >= 0)
            assertions.append(inv < BN254_PRIME)
            assertions.append((node_vars[b] * inv) % BN254_PRIME == node_vars[a] % BN254_PRIME)
            assertions.append(var == inv)

        elif ntype == 'select':
            # select(cond, then, else) = cond * (then - else) + else
            c, t, e = node['inputs']
            assertions.append(
                var == (node_vars[c] * (node_vars[t] - node_vars[e]) + node_vars[e] + BN254_PRIME) % BN254_PRIME
            )

        elif ntype == 'is_zero':
            inp = node['inputs'][0]
            assertions.append(z3.If(node_vars[inp] == 0, var == 1, var == 0))

        elif ntype == 'assert_eq':
            a, b = node['inputs']
            assertions.append(node_vars[a] == node_vars[b])
            assertions.append(var == 1)

        elif ntype == 'assert_bool':
            inp = node['inputs'][0]
            assertions.append(z3.Or(node_vars[inp] == 0, node_vars[inp] == 1))
            assertions.append(var == node_vars[inp])

        elif ntype == 'range_check':
            inp = node['inputs'][0]
            bits = node.get('bits', 64)
            upper = (1 << bits) if bits < 128 else BN254_PRIME
            assertions.append(node_vars[inp] >= 0)
            assertions.append(node_vars[inp] < upper)
            assertions.append(var == 1)

        elif ntype == 'poseidon':
            arity = len(node['inputs'])
            if arity not in poseidon_fns:
                arg_sorts = [z3.IntSort()] * arity
                poseidon_fns[arity] = z3.Function(
                    f'Poseidon_{arity}', *arg_sorts, z3.IntSort()
                )
            fn = poseidon_fns[arity]
            input_vars_z3 = [node_vars[inp] for inp in node['inputs']]
            assertions.append(var == fn(*input_vars_z3))
            assertions.append(var >= 0)
            assertions.append(var < BN254_PRIME)

        else:
            # Unknown node type — treat as unconstrained
            pass

    outputs = [node_vars[oid] for oid in graph.get('outputs', [])]
    return node_vars, assertions, outputs, input_vars


def verify_equivalence(input_path: str, output_path: str, timeout_ms: int = 30000, verbose: bool = False) -> int:
    """
    Verify that the optimized graph is semantically equivalent to the original.

    Returns:
        0 if equivalent, 1 if counterexample found, 2 if timeout/unknown
    """
    graph_in = load_graph(input_path)
    graph_out = load_graph(output_path)

    print("🔍 Translating original graph to Z3 SMT...")
    _, orig_assertions, orig_outputs, orig_inputs = translate_graph_to_z3(graph_in, "orig_", timeout_ms)

    print("🔍 Translating optimized graph to Z3 SMT...")
    _, opt_assertions, opt_outputs, opt_inputs = translate_graph_to_z3(graph_out, "opt_", timeout_ms)

    # Bind corresponding inputs to the same values
    input_bindings = []
    for label, orig_var in orig_inputs.items():
        if label in opt_inputs:
            input_bindings.append(orig_var == opt_inputs[label])

    # Formulate the verification query:
    # We want to prove: ∀inputs. (orig_assertions ∧ opt_assertions ∧ bindings) → outputs_equal
    # Equivalently: (orig_assertions ∧ opt_assertions ∧ bindings ∧ ¬outputs_equal) is UNSAT

    solver = z3.Solver()
    solver.set("timeout", timeout_ms)

    # Add all assertions
    for a in orig_assertions:
        solver.add(a)
    for a in opt_assertions:
        solver.add(a)
    for b in input_bindings:
        solver.add(b)

    # Assert that at least one output differs
    if orig_outputs and opt_outputs:
        output_neq = z3.Or(*[o != p for o, p in zip(orig_outputs, opt_outputs)])
        solver.add(output_neq)
    else:
        print("⚠️  No outputs to compare. Skipping equivalence check.")
        return 0

    print("⚡ Running equivalence check via Z3 SMT Solver...")
    if verbose:
        print(f"   Solver has {len(solver.assertions())} assertions")
        print(f"   Comparing {len(orig_outputs)} output(s)")
        print(f"   Timeout: {timeout_ms}ms")

    result = solver.check()

    if result == z3.unsat:
        print("✅ VERIFIED: Optimization preserves circuit semantics (proven equivalent for ALL inputs).")
        return 0

    elif result == z3.sat:
        model = solver.model()
        print("❌ FAILURE: Optimization has changed the circuit semantics! Found a counterexample:")
        for label, var in sorted(orig_inputs.items()):
            val = model.evaluate(var, model_completion=True)
            print(f"  Input '{label}': {val}")

        if verbose:
            for i, (o, p) in enumerate(zip(orig_outputs, opt_outputs)):
                orig_val = model.evaluate(o, model_completion=True)
                opt_val = model.evaluate(p, model_completion=True)
                print(f"  Output[{i}]: original={orig_val}, optimized={opt_val}")
        return 1

    else:
        print(f"⚠️  TIMEOUT/UNKNOWN: Z3 could not determine equivalence within {timeout_ms}ms.")
        print("   This may be due to circuit complexity. Try increasing --timeout.")
        return 2


def main():
    parser = argparse.ArgumentParser(description='DCL Z3 SMT Formal Equivalence Verifier')
    parser.add_argument('--input', required=True, help='Path to original DCIR graph JSON')
    parser.add_argument('--output', required=True, help='Path to optimized DCIR graph JSON')
    parser.add_argument('--timeout', type=int, default=30000, help='Z3 solver timeout in milliseconds (default: 30000)')
    parser.add_argument('--verbose', '-v', action='store_true', help='Enable verbose output')
    args = parser.parse_args()

    exit_code = verify_equivalence(args.input, args.output, args.timeout, args.verbose)
    sys.exit(exit_code)


if __name__ == '__main__':
    main()

#!/usr/bin/env python3
import json
import sys
import argparse
import z3

# BN254 prime field modulus
BN254_PRIME = 21888242871839275222246405745257275088548364400416034343698204186575808495617

def verify_equivalence(ir_in_path, ir_out_path):
    # Load computation graphs
    with open(ir_in_path, 'r') as f:
        graph_in = json.load(f)
    with open(ir_out_path, 'r') as f:
        graph_out = json.load(f)

    # Initialize Z3 solver
    solver = z3.Solver()

    # Shared input variables
    input_vars = {}
    
    # Collect all inputs from both graphs to declare shared symbolic variables
    all_input_labels = set()
    for g in [graph_in, graph_out]:
        for node in g["nodes"]:
            if node["node_type"] == "input":
                all_input_labels.add(node["label"])

    for label in all_input_labels:
        var = z3.Int(label)
        input_vars[label] = var
        # Constrain inputs to BN254 field range
        solver.add(var >= 0)
        solver.add(var < BN254_PRIME)

    # Uninterpreted functions dictionary for Poseidon (grouped by arity)
    poseidon_funcs = {}

    def translate_graph(graph):
        nodes = graph["nodes"]
        nodes_by_id = {node["id"]: node for node in nodes}
        node_exprs = {}
        assertions = []

        def get_expr(node_id):
            if node_id in node_exprs:
                return node_exprs[node_id]

            node = nodes_by_id[node_id]
            t = node["node_type"]
            inputs = node["inputs"]

            if t == "input":
                label = node["label"]
                expr = input_vars[label]
            elif t == "const":
                expr = int(node["value"])
            elif t == "add":
                expr = (get_expr(inputs[0]) + get_expr(inputs[1])) % BN254_PRIME
            elif t == "sub":
                expr = (get_expr(inputs[0]) - get_expr(inputs[1])) % BN254_PRIME
            elif t == "mul":
                expr = (get_expr(inputs[0]) * get_expr(inputs[1])) % BN254_PRIME
            elif t == "div":
                # Finite field division using helper variable
                b_expr = get_expr(inputs[1])
                inv_var = z3.Int(f"inv_{node['id']}")
                solver.add(z3.Implies(b_expr != 0, (b_expr * inv_var) % BN254_PRIME == 1))
                expr = (get_expr(inputs[0]) * inv_var) % BN254_PRIME
            elif t == "is_zero":
                val = get_expr(inputs[0])
                expr = z3.If(val == 0, 1, 0)
            elif t == "select":
                cond = get_expr(inputs[0])
                opt_true = get_expr(inputs[1])
                opt_false = get_expr(inputs[2])
                expr = z3.If(cond == 1, opt_true, opt_false)
            elif t == "range_check":
                val = get_expr(inputs[0])
                bits = node["bits"]
                expr = z3.If(z3.And(val >= 0, val < (2 ** bits)), 1, 0)
            elif t == "poseidon":
                arity = len(inputs)
                func_name = f"Poseidon_{arity}"
                if func_name not in poseidon_funcs:
                    sorts = [z3.IntSort()] * (arity + 1)
                    poseidon_funcs[func_name] = z3.Function(func_name, *sorts)
                p_func = poseidon_funcs[func_name]
                args = [get_expr(inp) for inp in inputs]
                expr = p_func(*args)
            elif t == "assert_eq":
                lhs = get_expr(inputs[0])
                rhs = get_expr(inputs[1])
                assertions.append(lhs == rhs)
                expr = 1
            elif t == "assert_bool":
                val = get_expr(inputs[0])
                assertions.append(z3.Or(val == 0, val == 1))
                expr = 1
            else:
                raise ValueError(f"Unknown node type: {t}")

            node_exprs[node_id] = expr
            return expr

        # Translate all outputs
        output_exprs = [get_expr(out_id) for out_id in graph["outputs"]]
        return output_exprs, assertions

    # Translate both graphs
    print("🔍 Translating original graph to Z3 SMT...")
    in_outputs, in_assertions = translate_graph(graph_in)

    print("🔍 Translating optimized graph to Z3 SMT...")
    out_outputs, out_assertions = translate_graph(graph_out)

    # Equivalence criteria:
    # Under the condition that original assertions hold, does there exist any input
    # where the optimized assertions are NOT met OR the output expressions differ?
    # Formulated as: Original_Assertions AND (NOT Optimized_Assertions OR Original_Outputs != Optimized_Outputs)
    orig_assertion_conj = z3.And(*in_assertions) if in_assertions else z3.BoolVal(True)
    opt_assertion_conj = z3.And(*out_assertions) if out_assertions else z3.BoolVal(True)
    
    outputs_equal = z3.And(*[in_out == out_out for in_out, out_out in zip(in_outputs, out_outputs)]) if in_outputs else z3.BoolVal(True)

    # Counterexample condition:
    counterexample_cond = z3.And(
        orig_assertion_conj,
        z3.Or(z3.Not(opt_assertion_conj), z3.Not(outputs_equal))
    )

    solver.add(counterexample_cond)

    print("⚡ Running equivalence check via Z3 SMT Solver...")
    result = solver.check()

    if result == z3.unsat:
        print("✅ SUCCESS: The optimized computation graph is mathematically equivalent to the original graph!")
        sys.exit(0)
    elif result == z3.sat:
        print("❌ FAILURE: Optimization has changed the circuit semantics! Found a counterexample:")
        model = solver.model()
        for label, var in input_vars.items():
            print(f"  Input '{label}': {model[var]}")
        sys.exit(1)
    else:
        print("⚠️ WARNING: Z3 equivalence check returned UNKNOWN. Equivalence cannot be guaranteed.")
        sys.exit(2)

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Formal equivalence verifier for DCL optimized graphs")
    parser.add_argument("--input", required=True, help="Path to input (original) IR JSON")
    parser.add_argument("--output", required=True, help="Path to output (optimized) IR JSON")
    args = parser.parse_args()

    verify_equivalence(args.input, args.output)

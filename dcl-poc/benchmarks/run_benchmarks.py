"""
DCL PoC Benchmark Suite.

Runs all benchmark circuits through the differentiable optimizer
and compares baseline vs optimized constraint counts.

Usage:
    python -m benchmarks.run_benchmarks
"""

from __future__ import annotations

import sys
import os

# Ensure project root is on path
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from dcl_poc.ir.graph import DCIRGraph
from dcl_poc.optimizer.engine import DifferentiableOptimizer, OptimizationConfig
from dcl_poc.verify.random_test import verify_equivalence_random

# Import benchmark circuits
from dcl_poc.circuits.range_proof import (
    build_range_proof,
    build_multi_range_proof,
    build_comparison_circuit,
)
from dcl_poc.circuits.poseidon import (
    build_poseidon_single,
    build_poseidon_chain,
)
from dcl_poc.circuits.merkle import (
    build_merkle_proof,
    build_merkle_proof_with_range,
)


def run_benchmark(
    name: str,
    graph: DCIRGraph,
    config: OptimizationConfig,
) -> dict:
    """Run a single benchmark and return results."""
    print(f"\n{'='*60}")
    print(f"  Benchmark: {name}")
    print(f"{'='*60}")

    # Print graph summary
    summary = graph.summary()
    print(f"  Graph: {summary['total_nodes']} nodes, "
          f"{summary['optimizable_nodes']} optimizable, "
          f"depth={summary['depth']}")

    # Run optimizer
    optimizer = DifferentiableOptimizer(config)
    result = optimizer.optimize(graph)

    # Verify equivalence
    passed, num_tests = verify_equivalence_random(graph, result.selections)
    print(f"  Verification: {'✅ PASSED' if passed else '❌ FAILED'} ({num_tests} tests)")

    return {
        "name": name,
        "nodes": summary["total_nodes"],
        "optimizable": summary["optimizable_nodes"],
        "baseline": result.baseline_constraints,
        "optimized": result.optimized_constraints,
        "reduction_pct": result.reduction_pct,
        "time_s": result.elapsed_seconds,
        "verified": passed,
    }


def print_results_table(results: list[dict]):
    """Print a formatted comparison table."""
    print(f"\n\n{'='*80}")
    print(f"  📊  DCL PoC — Differentiable Circuit Optimization Results")
    print(f"{'='*80}")

    # Header
    print(f"\n  {'Circuit':<30} {'Baseline':>10} {'Optimized':>10} {'Reduction':>10} {'Time':>8} {'Verified':>8}")
    print(f"  {'-'*30} {'-'*10} {'-'*10} {'-'*10} {'-'*8} {'-'*8}")

    for r in results:
        verified_str = "✅" if r["verified"] else "❌"
        print(
            f"  {r['name']:<30} "
            f"{r['baseline']:>10.0f} "
            f"{r['optimized']:>10.0f} "
            f"{r['reduction_pct']:>9.1f}% "
            f"{r['time_s']:>7.2f}s "
            f"{verified_str:>8}"
        )

    # Summary
    total_baseline = sum(r["baseline"] for r in results)
    total_optimized = sum(r["optimized"] for r in results)
    total_reduction = (1 - total_optimized / total_baseline) * 100 if total_baseline > 0 else 0

    print(f"  {'-'*30} {'-'*10} {'-'*10} {'-'*10} {'-'*8} {'-'*8}")
    print(f"  {'TOTAL':<30} {total_baseline:>10.0f} {total_optimized:>10.0f} {total_reduction:>9.1f}%")
    print()


def main():
    """Run all benchmarks."""
    print("🔮 DCL PoC — Differentiable Cryptographic Language")
    print("   Phase 0: Proof of Concept Benchmark Suite")
    print("   Testing: Can gradient descent optimize ZKP circuits?")

    config = OptimizationConfig(
        max_epochs=300,
        learning_rate=0.05,
        tau_start=5.0,
        tau_end=0.05,
        w_constraints=1.0,
        w_depth=0.1,
        w_noise=0.05,
        w_entropy=0.05,
        log_interval=100,
        verbose=True,
    )

    # Define benchmarks
    benchmarks = [
        ("Range Proof (8-bit)", build_range_proof(8)),
        ("Range Proof (32-bit)", build_range_proof(32)),
        ("Multi Range (4×16-bit)", build_multi_range_proof(4, 16)),
        ("Comparison (32-bit)", build_comparison_circuit(32)),
        ("Poseidon (2 inputs)", build_poseidon_single(2)),
        ("Poseidon Chain (4)", build_poseidon_chain(4)),
        ("Merkle Proof (d=4)", build_merkle_proof(4)),
        ("Merkle Proof (d=8)", build_merkle_proof(8)),
        ("Merkle+Range (d=4, 64b)", build_merkle_proof_with_range(4, 64)),
    ]

    results = []
    for name, graph in benchmarks:
        r = run_benchmark(name, graph, config)
        results.append(r)

    print_results_table(results)

    # Summary verdict
    all_positive = all(r["reduction_pct"] > 0 for r in results if r["optimizable"] > 0)
    if all_positive:
        print("  🎉 SUCCESS: Differentiable optimization consistently reduces constraints!")
        print("     The core hypothesis of DCL is validated.")
    else:
        print("  ⚠️  Mixed results. Some circuits did not benefit from optimization.")
        print("     Review loss function weights and cost model accuracy.")


if __name__ == "__main__":
    main()

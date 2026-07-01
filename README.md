# Differentiable Cryptographic Language (DCL)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/Rust-1.70+-orange.svg)](https://www.rust-lang.org/)
[![Python JAX](https://img.shields.io/badge/Python-JAX-blue.svg)](https://github.com/google/jax)

DCL is a domain-specific programming language designed for privacy-preserving computations (ZKP / FHE). By embedding automatic differentiation directly into the compiler pipeline, DCL compiles high-level math declarations into a Fully Differentiable Intermediate Representation (DCIR). It then utilizes gradient descent to automatically search for circuit structures that minimize R1CS constraints or homomorphic encryption (FHE) noise growth.

For the formal BNF grammar, type systems, and semantics of the language, refer to the [LANGUAGE_SPEC.md](file:///Users/liuyukai/CREATE/auv/dcl/LANGUAGE_SPEC.md).

---

## Technical Features and Innovations

### 1. Fully Differentiable Intermediate Representation (DCIR)
DCL represents computations as a differentiable Directed Acyclic Graph (DAG). Every node contains computing semantics and a set of learnable continuous relaxation parameters ($\alpha, \beta, \gamma$) adjusted by the compiler optimizer:
*   $\alpha$: Controls structural strategy selection (e.g., implementing a Range Proof via bit decomposition, lookup tables, or polynomial approximations).
*   $\beta$: Controls precision vs. noise trade-offs in FHE scenarios.
*   $\gamma$: Controls sub-circuit inlining/folding decisions.

### 2. Differentiable Optimizer Engine
The optimization passes of traditional compilers are replaced with a continuous Loss Function Minimization + Gradient Descent loop:

$$\mathcal{L}_{\text{total}} = w_1 \cdot \mathcal{L}_{\text{constraints}} + w_2 \cdot \mathcal{L}_{\text{noise}} + w_3 \cdot \mathcal{L}_{\text{depth}} + w_4 \cdot \mathcal{L}_{\text{correctness}}$$

*   $\mathcal{L}_{\text{constraints}}$: Minimizes the number of non-linear operations (multiplication gates in R1CS).
*   $\mathcal{L}_{\text{noise}}$: Tracks and minimizes noise propagation for Homomorphic Encryption (FHE).
*   $\mathcal{L}_{\text{depth}}$: Limits proof depth to optimize verification time.
*   $\mathcal{L}_{\text{correctness}}$: Enforces mathematical equivalence between the optimized circuit and the original program through SMT/random verification checks.

### 3. Static Information Flow and Secrecy Analysis
To prevent secret leaks in zero-knowledge circuits, the compiler implements a static taint analysis pass:
*   **Taint Propagation**: Input parameters declared as `private` are marked as `Secret`, while constants and `public` parameters are marked as `Public`. Gates propagate `Secret` status if any of their inputs are secret.
*   **Declassification**: Cryptographic one-way functions, such as `poseidon`, act as declassifiers and output `Public` status.
*   **Security Warnings**: If a `Secret` value flows directly into a public circuit output, the compiler triggers a compile-time warning:
    `[Security Warning]: Private secret from input(s) 'x' leaks directly to public output in circuit 'y'. Consider passing secrets through a one-way hash function (like poseidon) before exporting.`

### 4. Conditional Assertions inside Zero-Knowledge Circuits
Traditional compilers struggle with assertions nested within conditional branches (`if`/`else`). DCL resolves this in the lowerer using a path condition stack:
*   The compiler tracks active branch conditions and computes the path condition $P$ by multiplying the conditions of all parent branches.
*   An assertion `assert expr;` inside a branch is compiled into a constraint:
    $$P \cdot (1 - \text{lower}(\text{expr})) \equiv 0$$
*   If the execution path is inactive ($P = 0$), the constraint is trivially satisfied, preventing unexpected proving/verification failures on inactive paths.

### 5. Secure Division Constraints
To prevent soundness issues and division-by-zero vulnerabilities in the compiled ZK circuits, the Circom backend generates an inverse signal constraint for division operations (`NodeType::Div`):
```circom
signal inv_div_node_id;
inv_div_node_id <-- b == 0 ? 0 : 1 / b;
b * inv_div_node_id === 1;
n_node_id <-- a / b;
n_node_id * b === a;
```
If the divisor `b` is zero, the constraint `b * inv_div_node_id === 1` fails, ensuring that division by zero is safely blocked at the constraint level.

### 6. Diagnostic Recovery in Frontend
The parser and type checker support diagnostic recovery:
*   Instead of aborting on the first syntax or type error, the frontend collects multiple errors in a single compilation run.
*   If an expression has type errors or undefined variables, the compiler logs the error, falls back to the default `Type::Field`, and continues semantic analysis of the remaining codebase.

---

## Project Structure

The repository is organized as follows:

```
DCL-Differentiable-Cryptographic-Language/
├── dcl/                      # Production Rust implementation of the compiler
│   ├── LANGUAGE_SPEC.md      # Formal BNF grammar, type systems, and semantics
│   ├── crates/
│   │   ├── dcl-frontend/     # Lexer, Parser, AST, and Type Checker (with error recovery)
│   │   ├── dcl-ir/           # DCIR graph, lowerer with condition stacks, and secrecy checks
│   │   ├── dcl-codegen/      # Backends (R1CS, ACIR, and TFHE/FHE with secure division)
│   │   └── dcl-cli/          # Command-line interface
│   ├── dcl-optimizer/        # Python-integrated optimization hooks (optimize.py, verify.py)
│   ├── stdlib/               # DCL standard library (crypto, math, utils)
│   └── examples/             # ZK and FHE circuit examples
│
├── dcl-poc/                  # Phase 0: Python-based JAX Proof of Concept (PoC)
│   ├── dcl_poc/              # Core PoC modules (ir, optimizer, backends, verify)
│   ├── benchmarks/           # Benchmark scripts comparing optimization vs Circom --O2
│   └── requirements.txt      # Python dependencies for the PoC
│
├── editors/                  # Editor support
│   └── vscode/               # VS Code extension for syntax highlighting and LSP
│
└── README.md                 # Project Overview (English)
```

---

## Standard Library Modules

DCL provides built-in utilities in its standard library:
*   `std::crypto`: Algebraic hash functions (`poseidon`) and Merkle tree path validation (`verify_merkle`).
*   `std::fixed`: Fixed-point scaling math (scaled by $2^{16} = 65536$), including addition, subtraction, multiplication, division, and comparison operators (`gte`, `lte`).
*   `std::utils`: Bound constraints (`range_check` and `assert_in_range`).

---

## Syntax Example

Developers write mathematical logic and privacy constraints. The compiler automatically translates and optimizes the layout.

```dcl
use std::crypto;
use std::utils;

module AgeVerification

type Credential = {
    age:     Field,        // Private: prover's age
    id_hash: Field,        // Private: hash of prover's ID
}

circuit verify_adult(
    private cred: Credential,   // Hidden from Verifier
    public  threshold: Field,   // Visible to Verifier
) -> bool {

    // Range assertion
    assert cred.age >= threshold;

    // ZK-friendly hash function
    let computed_hash = crypto::poseidon(cred.age, cred.id_hash);

    // Returns verification status
    return computed_hash == cred.id_hash;
}
```

---

## Quick Start (Phase 0 PoC)

To verify the core hypothesis that gradient descent can find circuit configurations with fewer constraints than traditional compiler optimizers:

1.  **Navigate to the PoC folder & set up virtual environment**:
    ```bash
    cd dcl-poc
    python -m venv .venv
    source .venv/bin/activate
    ```

2.  **Install dependencies**:
    ```bash
    pip install -r requirements.txt
    ```

3.  **Run Benchmarks**:
    ```bash
    python -m benchmarks.run_benchmarks
    ```
    This script runs the JAX-based optimization loop on standard circuits (Poseidon, Range Proofs, and Merkle Trees) and displays the constraint count comparisons.

---

## Development Roadmap

*   **Phase 0: Proof of Concept (PoC)** (Current): Validate the gradient-based optimization on JAX benchmarks. Prove advantages over static heuristics (e.g., Circom --O2).
*   **Phase 1: Language Frontend**: Freeze syntax specs. Complete Rust lexer, parser, type checker, and lowering to AST.
*   **Phase 2: Compiler Core**: Implement Rust autograd engine, Gumbel-Softmax discretization, Adam optimization pass, and equivalence verification with Z3 SMT solvers.
*   **Phase 3: Backends & Ecosystem**: Target R1CS (arkworks), ACIR (Noir compatibility), and TFHE-rs (FHE). Deliver VS Code syntax highlighter and complete CLI tooling.

---

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE) for details.

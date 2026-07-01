# 🔮 Differentiable Cryptographic Language (DCL)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/Rust-1.70+-orange.svg)](https://www.rust-lang.org/)
[![Python JAX](https://img.shields.io/badge/Python-JAX-blue.svg)](https://github.com/google/jax)

DCL is an innovative programming language designed for privacy-preserving computations (ZKP / FHE). By embedding **automatic differentiation** directly into the compiler pipeline, DCL compiles high-level math declarations into a **Fully Differentiable Intermediate Representation (DCIR)**. It then utilizes gradient descent to automatically search for circuit structures that minimize R1CS constraints or homomorphic encryption (FHE) noise growth.

---

## 🚀 Key Innovations

### 1. Fully Differentiable Intermediate Representation (DCIR)
Unlike traditional static compiler intermediate representations, DCIR represents the calculation as a differentiable Directed Acyclic Graph (DAG). Every node contains not only computing semantics but also a set of **learnable continuous relaxation parameters** ($\alpha, \beta, \gamma$) adjusted by the compiler optimizer:
*   $\alpha$: Controls structural strategy selection (e.g., whether to implement a Range Proof via bit decomposition, lookup tables, or polynomial approximations).
*   $\beta$: Controls precision vs. noise trade-offs in FHE scenarios.
*   $\gamma$: Controls sub-circuit inlining/folding decisions.

### 2. Differentiable Optimizer Engine
The optimization passes of traditional compilers (heuristics, greedy matching) are replaced with a continuous **Loss Function Minimization + Gradient Descent** loop:

$$\mathcal{L}_{\text{total}} = w_1 \cdot \mathcal{L}_{\text{constraints}} + w_2 \cdot \mathcal{L}_{\text{noise}} + w_3 \cdot \mathcal{L}_{\text{depth}} + w_4 \cdot \mathcal{L}_{\text{correctness}}$$

*   $\mathcal{L}_{\text{constraints}}$: Minimizes the number of non-linear operations (multiplication gates in R1CS).
*   $\mathcal{L}_{\text{noise}}$: Tracks and minimizes noise propagation for Homomorphic Encryption (FHE).
*   $\mathcal{L}_{\text{depth}}$: Limits proof depth to optimize verification time.
*   $\mathcal{L}_{\text{correctness}}$: Enforces mathematical equivalence between the optimized circuit and the original program through SMT/random verification checks.

### 3. Differentiable Discrete Choice (Gumbel-Softmax)
DCL uses the **Gumbel-Softmax** trick to make discrete compiler decisions (such as picking a Range Proof algorithm) fully differentiable. Over epochs, a temperature parameter $\tau$ is annealed to 0, smoothly moving from a soft probability distribution of strategies to a concrete, optimized hard-gate execution path.

---

## 📂 Project Structure

The repository is organized as follows:

```
DCL-Differentiable-Cryptographic-Language/
├── dcl/                      # Production Rust implementation of the compiler
│   ├── crates/
│   │   ├── dcl-frontend/     # Lexer, Parser (chumsky), AST, and Type Checker
│   │   ├── dcl-ir/           # DCIR graph representation and metadata definitions
│   │   ├── dcl-codegen/      # Backends (R1CS, ACIR/Noir, and TFHE/FHE)
│   │   └── dcl-cli/          # Command-line interface (dcl build / prove / verify)
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
├── differentiable_cryptographic_language_framework.md # Concept Paper & Tech Specs
└── README.md                 # Project Overview (English)
```

---

## 📝 Syntax Example

Developers write mathematical logic and privacy constraints. The compiler automatically translates and optimizes the layout.

```dcl
module AgeVerification

type Credential = {
    age:     Field,        // Private: prover's age
    id_hash: Field,        // Private: hash of prover's ID
}

// Entry circuit for proof generation
circuit verify_adult(
    private cred: Credential,   // Hidden from Verifier
    public  threshold: Field,   // Visible to Verifier
) -> bool {

    // Declarative range constraint: auto-lowered to optimal range proof strategy
    assert cred.age >= threshold

    // ZK-friendly hash function
    let computed_hash = poseidon(cred.age, cred.id_hash)

    // Returns boolean verification check
    return computed_hash == cred.id_hash
}
```

---

## ⚡ Quick Start (Phase 0 PoC)

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

## 🗺️ Development Roadmap

*   **Phase 0: Proof of Concept (PoC)** *(Current)*: Validate the gradient-based optimization on JAX benchmarks. Prove advantages over static heuristics (e.g., Circom `--O2`).
*   **Phase 1: Language Frontend**: Freeze syntax specs. Complete Rust lexer, parser, type checker, and lowering to AST.
*   **Phase 2: Compiler Core**: Implement Rust autograd engine, Gumbel-Softmax discretization, Adam optimization pass, and equivalence verification with SMT solvers.
*   **Phase 3: Backends & Ecosystem**: Target R1CS (arkworks), ACIR (Noir compatibility), and TFHE-rs (FHE). Deliver VS Code syntax highlighter and complete CLI tooling.

---

## 📄 License

This project is licensed under the MIT License. See [LICENSE](LICENSE) for details.

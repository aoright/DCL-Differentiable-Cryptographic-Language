# Human-AI Collaborative Hackathon Guide for DCL

In modern hackathons, speed of prototyping and compilation correctness are critical. DCL (Differentiable Cryptographic Language) combined with AI coding assistants (like Cursor, GitHub Copilot, or Claude) enables developer teams to build, check, and optimize Zero-Knowledge Proof (ZKP) and Fully Homomorphic Encryption (FHE) circuits rapidly.

This guide outlines a step-by-step workflow for leveraging AI tools alongside DCL to prototype, compile, and optimize circuits during hackathons.

---

## 1. Hackathon Setup

Lower the initial friction by configuring your AI assistants to understand DCL syntax:

### Step A: Configure Cursor Rules
Ensure the [.cursorrules](file:///Users/liuyukai/CREATE/auv/.cursorrules) file is placed in the root of your project directory. Cursor will automatically read this file and inject DCL syntax constraints, mutability parameters, and visibility modifier requirements into its context window.

### Step B: Setup AI Chat Projects (Claude / Custom GPTs)
If you are using standalone chat interfaces (like Claude Projects or ChatGPT), upload [dcl-context-library.md](file:///Users/liuyukai/CREATE/auv/dcl-context-library.md) as reference material. This serves as a unified context source containing the language specification, standard library signatures, and correct annotated examples.

---

## 2. Iteration Workflow

Follow this loop to write and refine circuits:

### Phase 1: Prototyping (AI Generation)
Use your AI assistant to generate the initial logic. Provide clear specifications of your inputs and outputs.
*   **Prompt Template:**
    ```text
    Write a DCL circuit that verifies whether a user holds a valid membership pass.
    - Inputs: private credential (membership_id, signature), public threshold.
    - Outputs: public bool.
    - Rules: Use std::crypto::poseidon to hash private credentials before outputting.
    ```

### Phase 2: Compiler Checks (Lint & Verify)
Save the generated code to a `.dcl` file and run the DCL check command in your terminal:
```bash
dcl check my_circuit.dcl
```
DCL's frontend compiler supports **diagnostic recovery**, meaning it collects all typechecker and syntax errors instead of aborting on the first error. 

If compilation fails:
1.  Copy the compiler console output.
2.  Paste it to your AI coding assistant.
3.  Ask: `"DCL compiler returned these errors. Please fix my_circuit.dcl."`

### Phase 3: Automatic Layout Optimization (Gradient Search)
This is DCL's unique advantage. Instead of manually tuning constraints and strategies (like bit decomposition vs. lookup tables), run the DCL optimization loop:
```bash
dcl compile my_circuit.dcl -o my_circuit.circom --epochs 50
```
The compiler optimizer will perform gradient descent (using Gumbel-Softmax discrete-choice relaxation) to automatically find the circuit structure with the minimum R1CS gate count.

### Phase 4: Integration with Standard ZK Pipelines
Once the optimal `.circom` circuit is compiled, compile it using standard Circom tools:
```bash
circom my_circuit.circom --r1cs --wasm --sym -o ./build
```
Use `snarkjs` to generate proofs and deploy Solidity verification smart contracts to complete your hackathon application.

---

## 3. Winning Hackathon Strategies

To impress judges, emphasize the engineering metrics of DCL:
*   **Highlight Constraint Reduction:** Benchmark your DCL compiled circuit against a baseline Circom circuit. Present the exact percentage of constraints saved (e.g., "DCL reduced multiplication gate constraints by 18%, accelerating prover times").
*   **Emphasize Security Audits:** Mention that the DCL compiler automatically ran a static information flow secrecy check (`check_information_flow()`) to verify that no private inputs leaked directly to public verifier coordinates, ensuring mathematical security.

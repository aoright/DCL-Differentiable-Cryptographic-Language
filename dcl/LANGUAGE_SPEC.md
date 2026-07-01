# DCL (Differentiable Cryptographic Language) Specification

DCL is a domain-specific programming language designed to unify and optimize computations for **Zero-Knowledge Proofs (ZKP)** and **Fully Homomorphic Encryption (FHE)**. It employs a frontend syntax resembling Rust and leverages differentiable programming (via JAX and Gumbel-Softmax continuous relaxation) to automatically choose the most efficient compilation strategies for non-linear operators.

---

## 1. Lexical Grammar

### 1.1 Comments
Single-line comments are supported using double forward slashes. They are ignored by the Lexer.
```dcl
// This is a comment
```

### 1.2 Keywords
The following terms are reserved keywords:
- **Declarations**: `module`, `circuit`, `type`, `let`, `mut`, `extern`, `use`
- **Control Flow**: `for`, `in`, `if`, `else`, `return`, `assert`
- **Primitive Types**: `Field`, `bool`
- **Visibility Modifiers**: `public`, `private`, `shared`

### 1.3 Literals & Operators
- **Boolean Literals**: `true`, `false`
- **Number Literals**: Decimal sequences of digits (unlimited precision).
- **Operators**:
  - Arithmetic: `+`, `-`, `*`, `/`
  - Relational: `==`, `!=`, `<`, `>`, `<=`, `>=`
  - Logical: `&&`, `||`, `!`
  - Scope: `::`

---

## 2. Syntax (Context-Free Grammar)

Below is the Syntactic Grammar of DCL represented in BNF:

```bnf
<Module>        ::= "module" <Ident> <StmtList>

<StmtList>      ::= <Stmt> | <Stmt> <StmtList>

<Stmt>          ::= "use" <Path> ";"
                  | "type" <Ident> "{" <FieldList> "}"
                  | "circuit" <Ident> "(" <ParamList> ")" "->" <Type> "{" <Block> "}"
                  | "extern" "circuit" <Ident> "(" <ParamList> ")" "->" <Type> ";"

<FieldList>     ::= <Ident> ":" <Type> | <Ident> ":" <Type> "," <FieldList>

<ParamList>     ::= <Param> | <Param> "," <ParamList>

<Param>         ::= <Visibility> <Ident> ":" <Type>

<Visibility>    ::= "public" | "private" | "shared"

<Type>          ::= "Field"
                  | "bool"
                  | <Ident>
                  | <Type> "[" <Number> "]"

<Block>         ::= <Statement> | <Statement> <Block>

<Statement>     ::= "let" ["mut"] <Ident> [":" <Type>] "=" <Expr> ";"
                  | "assert" <Expr> ";"
                  | <Expr> "=" <Expr> ";"
                  | "return" <Expr> ";"
                  | "for" <Ident> "in" <Expr> ".." <Expr> "{" <Block> "}"
                  | "if" <Expr> "{" <Block> "}" [ "else" "{" <Block> "}" ]
```

---

## 3. Type System & Semantics

DCL features a strong, static type system with type inference for local variables.

### 3.1 Types
- `Field`: Represents an element in the prime field $\mathbb{F}_p$ (specifically BN254 prime field for ZKP).
- `bool`: Boolean values (`true`, `false`).
- `Array`: Homogeneous sequences of fixed length, e.g., `Field[4]`.
- `Struct`: Named, product type grouping key-value fields.

### 3.2 Variable Mutability
- By default, variables are **immutable** (`let x = 5;`).
- Re-assignment requires the `mut` modifier:
  ```dcl
  let mut x = 5;
  x = 10; // Allowed
  ```
- Circuit parameters and loop index variables are always immutable.

### 3.3 Diagnostic Recovery
The DCL Type Checker collects multiple errors in a single compilation run:
- Undefined variables or type mismatches default to `Type::Field` and continue checking subsequent statements in the block.
- Spans (`line` and `col`) are tracked on all AST elements to report precise coordinates.

---

## 4. Intermediate Representation (DCIR)

DCL parses source code and lowers it into a directed acyclic computation graph (DCIR):

### 4.1 Node Types
- `Const`: Constants (e.g. `1`, `0`).
- `Input`: Leaf parameter variables.
- `Add`, `Sub`, `Mul`, `Div`: Arithmetic gates.
- `Select`: Multiplexer gate (`cond * (then_val - else_val) + else_val`).
- `IsZero`: Constraint checker.
- `AssertEq`: Equality constraint assertion.
- `RangeCheck`: Bound verification constraint.
- `Poseidon`: Cryptographic hash function.

### 4.2 Single Static Assignment (SSA) branch merges
Under conditional branches (`if/else`), both paths are compiled. Side effects (mutated variables) are merged at the branch exit using `Select` MUX nodes based on the conditional signal.

### 4.3 Conditional Assertions
Assertions inside conditional blocks are conditionalized to prevent unconditional failures:
- Path condition $P$ is computed by multiplying all active branch conditions.
- The assertion `assert X;` is lowered to the constraint:
  $$P \cdot (1 - \text{lower}(X)) \equiv 0$$
- This guarantees that if the path condition is false ($P = 0$), the assertion is trivially satisfied, bypassing execution failure.

---

## 5. Differentiable Optimizer

DCIR nodes are assigned multiple alternative realization strategies (e.g., bit decomposition vs. lookup tables for range checks).
1. **Continuous Relaxation**: Choice coefficients are modeled as continuous parameters $\alpha$ using the Gumbel-Softmax distribution:
   $$P(\text{Strategy}_i) = \frac{\exp((\alpha_i + g_i)/\tau)}{\sum_j \exp((\alpha_j + g_j)/\tau)}$$
2. **Gradient Descent**: The optimizer minimizes a multi-objective loss function balancing gate count, proving time, and depth:
   $$\mathcal{L} = w_1 \cdot \text{Constraints} + w_2 \cdot \text{Depth}$$
3. **Formal Equivalence**: The final selected strategy is translated to Z3 SMT solver rules to mathematically verify that the optimized graph behaves identically to the original unoptimized program on all inputs.

---

## 6. Standard Library

DCL distributes common utilities in standard library modules:

### 6.1 `std::crypto`
- `poseidon(x: Field, y: Field) -> Field`: Algebraic hash function.
- `verify_merkle(leaf: Field, path: Field[4], root: Field) -> bool`: Proof path verification.

### 6.2 `std::fixed`
Fixed-point representation math (scaled by $2^{16} = 65536$):
- `from_int(x: Field) -> Field`
- `to_int(x: Field) -> Field`
- `add(a: Field, b: Field) -> Field`
- `sub(a: Field, b: Field) -> Field`
- `mul(a: Field, b: Field) -> Field`
- `div(a: Field, b: Field) -> Field`
- `gte(a: Field, b: Field) -> bool`
- `lte(a: Field, b: Field) -> bool`

### 6.3 `std::utils`
- `range_check(value: Field, bits: Field) -> bool`
- `assert_in_range(x: Field, min: Field, max: Field) -> bool`

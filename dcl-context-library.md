# DCL (Differentiable Cryptographic Language) Context Library

DCL is a programming language designed to unify and optimize computations for Zero-Knowledge Proofs (ZKP) and Fully Homomorphic Encryption (FHE). It employs a Rust-like syntax and leverages differentiable programming to automatically search for the most efficient circuit structures for non-linear operators.

This library is a consolidated reference containing grammar, standard library signatures, and correct code examples for use in AI context windows.

---

## 1. Syntax and Grammar Specification

### 1.1 Keywords and Literals
- **Keywords**: `module`, `circuit`, `type`, `let`, `mut`, `extern`, `use`, `for`, `in`, `if`, `else`, `return`, `assert`, `Field`, `bool`, `public`, `private`, `shared`.
- **Literals**: `true`, `false`, decimal sequence digits (e.g., `123`), hexadecimal digits (e.g., `0xFF` - parsed and converted to decimal internally).
- **Operators**: `+`, `-`, `*`, `/`, `==`, `!=`, `<`, `>`, `<=`, `>=`, `&&`, `||`, `!`, `::`.

### 1.2 Backus-Naur Form (BNF) Grammar
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
<Type>          ::= "Field" | "bool" | <Ident> | <Type> "[" <Number> "]"
<Block>         ::= <Statement> | <Statement> <Block>
<Statement>     ::= "let" ["mut"] <Ident> [":" <Type>] "=" <Expr> ";"
                  | "assert" <Expr> ";"
                  | <Expr> "=" <Expr> ";"
                  | "return" <Expr> ";"
                  | "for" <Ident> "in" <Expr> ".." <Expr> "{" <Block> "}"
                  | "if" <Expr> "{" <Block> "}" [ "else" "{" <Block> "}" ]
                  | <Expr> ";"
```

### 1.3 Key Rules
- **Variables**: Immutable by default. Re-assignment requires the `mut` keyword.
- **Span Coordinates**: Tracked for all AST elements to support multi-diagnostic error collection in the type checker.
- **Conditional Assertions**: The compiler conditionalizes assertions inside `if` statements by multiplying with the path condition $P$ (e.g., `assert expr;` lowers to `P * (1 - expr) == 0`), preventing proving failures on inactive paths.

---

## 2. Standard Library Signatures

Always reference library components using their full namespace or import them at the top.

### 2.1 `std::crypto`
Provides cryptographic primitives.
```dcl
module std::crypto

// Poseidon algebraic hash function (ZK-friendly)
extern circuit poseidon(x: Field, y: Field) -> Field;

// Merkle tree path verification over 4 steps
circuit verify_merkle(
    private leaf: Field,
    private path: Field[4],
    public root: Field
) -> bool {
    let mut current = leaf;
    for i in 0..4 {
        current = poseidon(current, path[i]);
    }
    return current == root;
}
```

### 2.2 `std::fixed`
Provides fixed-point arithmetic (scaled by $2^{16} = 65536$).
```dcl
module std::fixed

extern circuit from_int(x: Field) -> Field;
extern circuit to_int(x: Field) -> Field;
extern circuit add(a: Field, b: Field) -> Field;
extern circuit sub(a: Field, b: Field) -> Field;

circuit mul(a: Field, b: Field) -> Field {
    let raw_mul = a * b;
    return raw_mul / 65536;
}

circuit div(a: Field, b: Field) -> Field {
    let scaled_a = a * 65536;
    return scaled_a / b;
}

circuit gte(a: Field, b: Field) -> bool {
    return a >= b;
}

circuit lte(a: Field, b: Field) -> bool {
    return a <= b;
}
```

### 2.3 `std::utils`
Provides bound constraints and helper circuits.
```dcl
module std::utils

extern circuit range_check(value: Field, bits: Field) -> bool;

circuit assert_in_range(x: Field, min: Field, max: Field) -> bool {
    assert x >= min;
    assert x <= max;
    return true;
}
```

---

## 3. Annotated Reference Examples

### Example 3.1: Age Verification (With Secrecy Hash)
Demonstrates struct nesting, parameter visibility, range check assertions, and hashing secrets to prevent information leaks.

```dcl
use std::crypto;
use std::utils;

module AgeVerification

type Credential = {
    age:     Field,        // Private actual age
    id_hash: Field,        // Private hash identifier
}

circuit verify_adult(
    private cred: Credential,   // Hidden from Verifier
    public  threshold: Field,   // Visible to Verifier
) -> bool {
    // Assert age is above or equal to threshold
    assert cred.age >= threshold;

    // Securely hash the secret age and id_hash using Poseidon
    // This sanitizes/declassifies the secret information
    let computed_hash = crypto::poseidon(cred.age, cred.id_hash);

    // Returns a public verification output
    return computed_hash == cred.id_hash;
}
```

### Example 3.2: Bounded Loops and Conditional Branch merging
Demonstrates conditional statements, variable mutability, and how DCL merges environment states at the end of conditional blocks.

```dcl
use std::utils;

module MathSelector

circuit select_and_bound(
    public cond: bool,
    public x: Field,
    public y: Field
) -> Field {
    let mut res = 0;
    
    if cond {
        res = x;
        // Conditional assertion: only enforced if cond is true
        assert x > 10;
    } else {
        res = y;
        // Conditional assertion: only enforced if cond is false
        assert y > 20;
    }
    
    return res;
}
```

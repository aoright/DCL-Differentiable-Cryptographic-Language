# Changelog

All notable changes to the DCL (Differentiable Cryptographic Language) project will be documented in this file.

## [0.3.0] - 2026-07-01

### Added
- **Constrained Integer Types**: `u8`, `u16`, `u32`, `u64` types that compile to `Field + RangeCheck(n)` at IR level
- **Adam Optimizer**: Upgraded from plain SGD to Adam (β₁=0.9, β₂=0.999) with bias-corrected moments
- **Division-by-Zero Detection**: Static security analysis now detects constant and input-driven division by zero
- **Shared Visibility**: `shared` inputs now have dedicated IR semantics (treated as secret for IFC)
- **Multi-Output TFHE**: FHE backend supports circuits with multiple return values
- **Common Subexpression Elimination (CSE)**: Merges identical computation nodes within DCIR graphs to reduce compiler constraints
- **Extended Constant Folding**: Now handles `Div` (integer division) and `IsZero` (constant propagation)
- **17 IR Unit Tests**: Covering constant folding, DCE, information flow analysis, and lowerer correctness
- **12 Codegen Unit Tests**: Covering Circom and TFHE output structure and correctness
- **Stdlib Expansion**: New `hash.dcl` (Poseidon wrappers, commitments, nullifiers) and `comparators.dcl` (equality, ordering, range assertions)
- **Binary Safety Guards**: All `bits.dcl` operations now enforce binary input constraints via `x*x == x`
- **TFHE Poseidon**: Improved from stub to LUT-based programmable bootstrapping with detailed documentation
- **Mermaid Architecture Diagram**: README now includes visual pipeline diagram

### Changed
- **`abs()` → `abs_diff()`**: Renamed and redesigned for finite field safety with proper documentation
- **DCE Performance**: Switched from O(N) linear search to O(1) HashMap-based node lookup
- **Hex Literal Parsing**: Fixed overflow by using `BigUint` instead of `u128` (supports 254-bit BN254 values)
- **Z3 Precision**: Fixed `int(float(val_str))` → `int(val_str)` to prevent precision loss for large constants
- **Return Type Check**: Fixed bug where circuits returning `Field` could silently skip missing return statements
- **Type Compatibility**: Uint types are now compatible with Field in assignments and operations

### Fixed
- Lexer: `u128` overflow for hex literals > 128 bits
- TypeChecker: Missing return statement detection for non-Bool circuits
- Z3 Verifier: Precision loss for constants > 2^53
- TFHE Codegen: Single-output limitation

## [0.2.0] - 2026-06-30

### Added
- Block comments (`/* */`) with nesting support
- Hexadecimal literals (`0x...`)
- Unary negation operator (`-expr`)
- Span range tracking (4-tuple: start_line, start_col, end_line, end_col)
- "Did you mean?" suggestions for typos (Levenshtein distance ≤ 3)
- Information flow analysis (private→public leak detection)
- Under-constrained signal detection
- `dcl init` scaffolding command
- Fixed-point arithmetic stdlib (`std::fixed`)

## [0.1.0] - 2026-06-28

### Added
- Initial release
- Core language: circuits, structs, arrays, for loops, if/else
- Frontend: Lexer, Parser, TypeChecker
- DCIR: DAG graph with strategy annotations
- Optimizer: Gumbel-Softmax with JAX
- Verifier: Z3 SMT equivalence checking
- Backends: Circom (ZKP) and TFHE-rs (FHE)
- Standard library: crypto, math, bits, utils

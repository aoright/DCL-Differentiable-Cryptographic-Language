#!/bin/bash
set -e

# Setup directories
WORKSPACE_DIR="/Users/liuyukai/CREATE/auv/dcl"
BUILD_DIR="$WORKSPACE_DIR/examples/build_e2e"
rm -rf "$BUILD_DIR"
mkdir -p "$BUILD_DIR"

echo "=== Step 1: Compiling DCL to Circom ==="
cargo run --package dcl-cli -- compile "$WORKSPACE_DIR/examples/age_verify.dcl" -o "$WORKSPACE_DIR/examples/age_verify.circom"

echo "=== Step 2: Compiling Circom to R1CS and WASM ==="
# circom requires mock poseidon.circom in the include path or the same directory.
# Copy mock poseidon to the build directory along with age_verify.circom.
cp "$WORKSPACE_DIR/examples/poseidon.circom" "$BUILD_DIR/poseidon.circom"
cp "$WORKSPACE_DIR/examples/age_verify.circom" "$BUILD_DIR/age_verify.circom"

# Run circom compiler from the build directory
cd "$BUILD_DIR"
circom age_verify.circom --r1cs --wasm --sym -o .

echo "=== Step 3: Generating Input JSON ==="
cat <<EOF > input.json
{
    "cred_age": "21",
    "cred_id_hash": "12345",
    "threshold": "18"
}
EOF

echo "=== Step 4: Computing Witness ==="
node age_verify_js/generate_witness.js age_verify_js/age_verify.wasm input.json witness.wtns

echo "=== Step 5: Generating Local Powers of Tau ==="
npx snarkjs powersoftau new bn128 10 pot10_0000.ptau -v
npx snarkjs powersoftau contribute pot10_0000.ptau pot10_0001.ptau --name="Contributor 1" -v -e="some random entropy"
npx snarkjs powersoftau prepare phase2 pot10_0001.ptau pot10_final.ptau -v

echo "=== Step 6: Performing PLONK Setup ==="
npx snarkjs plonk setup age_verify.r1cs pot10_final.ptau circuit_final.zkey
npx snarkjs zkey export verificationkey circuit_final.zkey verification_key.json

echo "=== Step 7: Generating Proof ==="
npx snarkjs plonk prove circuit_final.zkey witness.wtns proof.json public.json

echo "=== Step 8: Verifying Proof ==="
npx snarkjs plonk verify verification_key.json public.json proof.json

echo "============================================="
echo "[OK] PLONK proof generated and verified successfully!"
echo "============================================="

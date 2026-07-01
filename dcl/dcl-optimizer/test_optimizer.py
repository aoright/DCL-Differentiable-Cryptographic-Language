import sys
import os
import pytest
import math

# Add the optimizer directory to python path
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import optimize
import verify

def test_cosine_lr():
    # Verify that learning rate starts near base and decays to min
    lr_start = optimize.cosine_lr(0.1, 0, 100, min_lr=0.01)
    lr_mid = optimize.cosine_lr(0.1, 50, 100, min_lr=0.01)
    lr_end = optimize.cosine_lr(0.1, 100, 100, min_lr=0.01)
    
    assert math.isclose(lr_start, 0.1, abs_tol=1e-5)
    assert lr_mid < 0.1
    assert lr_mid > 0.01
    assert math.isclose(lr_end, 0.01, abs_tol=1e-5)

def test_compute_topology_depth():
    # Minimal graph with 3 nodes: 0 (input), 1 (input) -> 2 (add)
    mock_graph = {
        "nodes": [
            {"id": 0, "inputs": []},
            {"id": 1, "inputs": []},
            {"id": 2, "inputs": [0, 1]}
        ]
    }
    depths = optimize.compute_topology_depth(mock_graph)
    assert depths[0] == 0
    assert depths[1] == 0
    assert depths[2] == 1

def test_gumbel_softmax_deterministic():
    import numpy as np
    alpha = np.array([1.0, 2.0, 3.0])
    # Very low temp should behave like argmax (deterministic)
    probs = optimize.gumbel_softmax(alpha, 0.01)
    assert np.argmax(probs) == 2
    assert math.isclose(probs[2], 1.0, abs_tol=1e-3)

def test_verify_bn254_prime():
    # Verify BN254 prime matches spec
    assert verify.BN254_PRIME == 21888242871839275222246405745257275088548364400416034343698204186575808495617

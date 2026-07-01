# DCL PoC — Differentiable Cryptographic Language (Phase 0)

> **验证核心假设**：梯度下降能否自动搜索到比 Circom `--O2` 约束更少的 ZKP 电路结构？

## 项目结构

```
dcl-poc/
├── dcl_poc/
│   ├── __init__.py
│   ├── ir/                  # DCIR: 可微中间表示
│   │   ├── __init__.py
│   │   ├── graph.py         # DAG 计算图
│   │   ├── nodes.py         # IR 节点定义
│   │   └── cost_model.py    # 各策略的成本模型
│   ├── optimizer/           # 可微优化引擎
│   │   ├── __init__.py
│   │   ├── loss.py          # 损失函数 L_total
│   │   ├── gumbel.py        # Gumbel-Softmax 离散化
│   │   └── engine.py        # 优化循环 (JAX)
│   ├── backends/            # 后端代码生成
│   │   ├── __init__.py
│   │   └── r1cs.py          # R1CS 约束生成 + 计数
│   ├── circuits/            # 基准测试电路
│   │   ├── __init__.py
│   │   ├── range_proof.py   # 范围证明电路
│   │   ├── poseidon.py      # Poseidon 哈希电路
│   │   └── merkle.py        # Merkle 树电路
│   └── verify/              # 等价性验证
│       ├── __init__.py
│       └── random_test.py   # L1: 随机输入测试
├── benchmarks/
│   └── run_benchmarks.py    # 基准测试脚本
├── requirements.txt
└── README.md
```

## 快速开始

```bash
cd dcl-poc
python -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt
python -m benchmarks.run_benchmarks
```

## 核心概念

### 1. DCIR 节点
每个计算节点携带**可学习松弛参数 α**，控制实现策略选择（如布尔分解 vs 查找表）。

### 2. 损失函数
```
L_total = w₁·L_constraints + w₂·L_depth + w₃·L_correctness
```

### 3. Gumbel-Softmax
使离散策略选择可微分，通过温度退火逐步从软选择过渡到硬选择。

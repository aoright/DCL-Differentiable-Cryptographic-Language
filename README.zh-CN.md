# 🔮 可微密码学语言 (DCL — Differentiable Cryptographic Language)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/Rust-1.70+-orange.svg)](https://www.rust-lang.org/)
[![Python JAX](https://img.shields.io/badge/Python-JAX-blue.svg)](https://github.com/google/jax)

DCL 是一门面向隐私计算（零知识证明 ZKP / 完全同态加密 FHE）的编译型编程语言。它创新性地将**自动微分（Automatic Differentiation）**嵌入到编译器管线中，将高层数学声明式逻辑编译为**完全可微的中间表示 (DCIR — Differentiable Cryptographic Intermediate Representation)**，并在编译期通过梯度下降自动搜索约束数量最少、噪声增长最慢的密码学电路结构。

---

## 🚀 核心技术创新

### 1. 完全可微的中间表示 (DCIR)
与传统的静态编译器中间表示不同，DCIR 是一个可微分的有向无环计算图 (DAG)。图中的每个计算节点除了携带本身的计算语义外，还携带一组**可学习的连续松弛参数**（learnable relaxation parameters $\alpha, \beta, \gamma$），用于在编译期被梯度优化器调整：
*   $\alpha$：控制结构策略的选择（例如：一个范围证明 Range Proof 是采用布尔分解、查找表，还是多项式近似来实现）。
*   $\beta$：控制同态加密 (FHE) 场景下的精度与噪声预算分配。
*   $\gamma$：控制子电路的内联与折叠决策。

### 2. 可微优化器引擎
DCL 将传统编译器依赖启发式 Pass 或贪心匹配的优化过程，替换为了一个端到端**可微的损失函数 (Loss Function) 与梯度下降 (Gradient Descent) 循环**：

$$\mathcal{L}_{\text{total}} = w_1 \cdot \mathcal{L}_{\text{constraints}} + w_2 \cdot \mathcal{L}_{\text{noise}} + w_3 \cdot \mathcal{L}_{\text{depth}} + w_4 \cdot \mathcal{L}_{\text{correctness}}$$

*   $\mathcal{L}_{\text{constraints}}$：最小化非线性运算（即 R1CS 中乘法门的数量）以降低约束总数。
*   $\mathcal{L}_{\text{noise}}$：追踪并最小化同态加密 (FHE) 中的密文噪声增长速度。
*   $\mathcal{L}_{\text{depth}}$：优化 DAG 的最长路径长度（最小化电路深度，从而降低验证开销）。
*   $\mathcal{L}_{\text{correctness}}$：在优化期间通过符号执行（Z3 SMT 求解器）和随机测试，保证优化前后的电路在数学上完全等价。

### 3. 可微离散策略选择 (Gumbel-Softmax)
DCL 引入了 **Gumbel-Softmax** 技巧来使离散的编译器决策（例如选择哪种密码学原语或分解策略）可微分。随着训练轮数 (Epoch) 的增加，通过对温度参数 $\tau$ 进行退火（使其逐渐趋近于 0），使原本由概率构成的软选择平滑地收敛为确定性的、最优的离散电路结构。

---

## 📂 项目结构

项目仓库的整体目录结构如下：

```
DCL-Differentiable-Cryptographic-Language/
├── dcl/                      # 基于 Rust 实现的编译器生产级代码
│   ├── crates/
│   │   ├── dcl-frontend/     # 前端词法分析、语法分析 (chumsky)、AST 及类型检查器
│   │   ├── dcl-ir/           # DCIR 计算图数据结构与松弛参数定义
│   │   ├── dcl-codegen/      # 后端代码生成（支持 R1CS, ACIR/Noir 以及 TFHE/FHE）
│   │   └── dcl-cli/          # 命令行工具 (dcl build / prove / verify)
│   ├── dcl-optimizer/        # Python 优化器接口（包含 optimize.py 和 verify.py）
│   ├── stdlib/               # DCL 密码学与数学标准库 (crypto, math, utils)
│   └── examples/             # 示例电路文件 (dcl, circom) 及端到端测试
│
├── dcl-poc/                  # Phase 0: 基于 Python 和 JAX 实现的概念验证 (PoC)
│   ├── dcl_poc/              # 可微计算图、Gumbel-Softmax 引擎及后端生成器
│   ├── benchmarks/           # 基准测试脚本，用于对比 DCL 与 Circom --O2 的约束数量
│   └── requirements.txt      # PoC 项目的 Python 依赖项
│
├── editors/                  # 编辑器支持
│   └── vscode/               # 支持语法高亮与 LSP 的 VS Code 插件
│
├── differentiable_cryptographic_language_framework.md # DCL 详细设计规格白皮书
└── README.md                 # 英文 README
```

---

## 📝 语法示例

开发者只需声明计算逻辑与隐私属性，编译器将自动处理后续的低层编译与电路结构优化。

```dcl
module AgeVerification

type Credential = {
    age:     Field,        // 私有：实际年龄
    id_hash: Field,        // 私有：身份信息哈希
}

// circuit 声明此函数将被编译为可证明的密码学电路
circuit verify_adult(
    private cred: Credential,   // 私有输入（Prover 可见，不暴露给 Verifier）
    public  threshold: Field,   // 公开输入（双方可见）
) -> bool {

    // 声明式约束：编译器自动搜索并应用最优的范围证明策略
    assert cred.age >= threshold

    // 使用零知识证明友好的 Poseidon 哈希
    let computed_hash = poseidon(cred.age, cred.id_hash)

    // 返回等价性约束结果
    return computed_hash == cred.id_hash
}
```

---

## ⚡ 快速开始 (Phase 0 PoC)

如果你想验证本项目的核心假设——即“梯度下降可以自动搜索到比传统编译器（如 Circom `--O2`）约束更少的电路结构”，请运行 PoC 阶段的代码：

1.  **进入 PoC 目录并创建虚拟环境**：
    ```bash
    cd dcl-poc
    python -m venv .venv
    source .venv/bin/activate
    ```

2.  **安装所需依赖项**：
    ```bash
    pip install -r requirements.txt
    ```

3.  **运行基准测试**：
    ```bash
    python -m benchmarks.run_benchmarks
    ```
    该脚本将通过 JAX 在 Poseidon 哈希、范围证明以及 Merkle 树电路上执行梯度优化循环，并输出优化前后的电路约束数量对比。

---

## 🗺️ 开发路线图

*   **Phase 0：概念验证 (PoC)** *(当前阶段)*：基于 Python + JAX 搭建最小化可微优化器。在三大 Benchmark 电路上证明相比静态启发式优化器（如 Circom `--O2`）具有更少的约束。
*   **Phase 1：语言前端**：完成 DCL v0.1 的语法规范制定，并使用 Rust 实现 Lexer, Parser, 类型检查器（Field 有限域类型及隐私修饰符）和 AST → DCIR 降低。
*   **Phase 2：编译器核心**：在 Rust 中实现完整的 DCIR 计算图、轻量级自动微分引擎、Gumbel-Softmax 离散化Pass，以及基于 SMT 求解器 (Z3) 的等价性验证。
*   **Phase 3：生态与扩展**：实现多后端生成（Arkworks R1CS、Noir 兼容的 ACIR、TFHE-rs），推出完整的 CLI 工具以及 VS Code 开发插件。

---

## 📄 开源许可

本项目采用 MIT 开源许可证。详见 [LICENSE](LICENSE) 文件。

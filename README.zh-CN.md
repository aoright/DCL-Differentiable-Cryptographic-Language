# 可微密码学语言 (DCL — Differentiable Cryptographic Language)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/Rust-1.70+-orange.svg)](https://www.rust-lang.org/)
[![Python JAX](https://img.shields.io/badge/Python-JAX-blue.svg)](https://github.com/google/jax)

DCL 是一门面向隐私计算（零知识证明 ZKP / 完全同态加密 FHE）的编译型编程语言。它创新性地将自动微分（Automatic Differentiation）嵌入到编译器管线中，将高层数学声明式逻辑编译为完全可微的中间表示 (DCIR — Differentiable Cryptographic Intermediate Representation)，并在编译期通过梯度下降自动搜索约束数量最少、噪声增长最慢的密码学电路结构。

关于该语言的正式 BNF 语法、类型系统和语义设计，请参阅 [LANGUAGE_SPEC.md](file:///Users/liuyukai/CREATE/auv/dcl/LANGUAGE_SPEC.md)。

---

## 技术特性与创新

### 1. 完全可微的中间表示 (DCIR)
DCL 将计算表示为一个可微分的有向无环计算图 (DAG)。图中的每个计算节点除了携带本身的计算语义外，还携带一组可学习的连续松弛参数（learnable relaxation parameters $\alpha, \beta, \gamma$），用于在编译期被梯度优化器调整：
*   $\alpha$：控制结构策略的选择（例如：一个范围证明 Range Proof 是采用布尔分解、查找表，还是多项式近似来实现）。
*   $\beta$：控制同态加密 (FHE) 场景下的精度与噪声预算分配。
*   $\gamma$：控制子电路的内联与折叠决策。

### 2. 可微优化器引擎
DCL 将传统编译器依赖启发式 Pass 或贪心匹配的优化过程，替换为了一个端到端可微的损失函数 (Loss Function) 与梯度下降 (Gradient Descent) 循环：

$$\mathcal{L}_{\text{total}} = w_1 \cdot \mathcal{L}_{\text{constraints}} + w_2 \cdot \mathcal{L}_{\text{noise}} + w_3 \cdot \mathcal{L}_{\text{depth}} + w_4 \cdot \mathcal{L}_{\text{correctness}}$$

*   $\mathcal{L}_{\text{constraints}}$：最小化非线性运算（即 R1CS 中乘法门的数量）以降低约束总数。
*   $\mathcal{L}_{\text{noise}}$：追踪并最小化同态加密 (FHE) 中的密文噪声增长速度。
*   $\mathcal{L}_{\text{depth}}$：优化 DAG 的最长路径长度（最小化电路深度，从而降低验证开销）。
*   $\mathcal{L}_{\text{correctness}}$：在优化期间通过符号执行（Z3 SMT 求解器）和随机测试，保证优化前后的电路在数学上完全等价。

### 3. 静态信息流与隐私安全分析 (Secrecy Check)
为了防止零知识证明电路中的隐私数据泄漏，编译器实现了一个静态污点分析 Pass：
*   **污点传播**：被声明为 `private` 的输入参数会被标记为 `Secret`（机密），常量及 `public` 参数被标记为 `Public`（公开）。如果节点的任一输入为机密，则该节点的输出也是机密。
*   **隐私去污/解密**：密码学单向函数（如 `poseidon` 哈希）具有去污作用，其输出将被视为 `Public`。
*   **安全警告**：如果一个 `Secret` 状态的变量未经过单向哈希直接流向了公开的电路输出，编译器会在编译阶段发出警告：
    `[Security Warning]: Private secret from input(s) 'x' leaks directly to public output in circuit 'y'. Consider passing secrets through a one-way hash function (like poseidon) before exporting.`

### 4. 零知识电路中的条件断言 (Conditional Assertions)
传统的零知识电路编译器难以处理嵌套在条件分支 (`if`/`else`) 内部的断言。DCL 的 Lowerer 通过引入条件路径栈解决了这一问题：
*   编译器追踪当前的条件分支，并通过连乘所有父分支的条件信号，计算出当前的分支路径条件 $P$。
*   分支内部的断言语句 `assert expr;` 会被编译为约束形式：
    $$P \cdot (1 - \text{lower}(\text{expr})) \equiv 0$$
*   如果执行路径不激活（即 $P = 0$），则该约束恒成立，从而防止未激活路径触发断言失败而导致整个证明或验证过程崩溃。

### 5. 安全的除法约束 (Secure Division)
为了防止编译生成的 ZK 电路发生健全性问题或除以零漏洞，Circom 后端为除法操作 (`NodeType::Div`) 自动生成倒数信号约束：
```circom
signal inv_div_node_id;
inv_div_node_id <-- b == 0 ? 0 : 1 / b;
b * inv_div_node_id === 1;
n_node_id <-- a / b;
n_node_id * b === a;
```
如果除数 `b` 为零，约束 `b * inv_div_node_id === 1` 将无法通过，从而在约束层面安全地拦截了除以零行为。

### 6. 前端诊断恢复 (Diagnostic Recovery)
编译器前端的语法解析器和类型检查器支持诊断恢复机制：
*   前端在遇到第一个语法或类型错误时不会直接中断编译，而是收集多个错误。
*   如果某个表达式存在类型错误或未定义变量，类型检查器会记录错误，并使用默认类型 `Type::Field` 替代以继续分析后续的变量和语句。

---

## 项目结构

项目仓库的整体目录结构如下：

```
DCL-Differentiable-Cryptographic-Language/
├── dcl/                      # 基于 Rust 实现的编译器生产级代码
│   ├── LANGUAGE_SPEC.md      # 语言的正式 BNF 语法、类型系统与语义规范
│   ├── crates/
│   │   ├── dcl-frontend/     # 前端词法分析、语法分析、AST 及支持错误恢复的类型检查器
│   │   ├── dcl-ir/           # DCIR 计算图数据结构、支持条件路径的 Lowerer 以及污点分析 Pass
│   │   ├── dcl-codegen/      # 后端代码生成（支持 R1CS, ACIR 以及内置安全除法约束的 Circom 生成）
│   │   └── dcl-cli/          # 命令行工具
│   ├── dcl-optimizer/        # Python 优化器接口（包含 optimize.py 和 verify.py）
│   ├── stdlib/               # DCL 标准库 (crypto, math, utils)
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
└── README.md                 # 英文 README
```

---

## 标准库模块

DCL 在标准库中预置了常用工具：
*   `std::crypto`：代数哈希函数 (`poseidon`) 与 Merkle 树路径有效性验证电路 (`verify_merkle`)。
*   `std::fixed`：固定点数运算（放大比例为 $2^{16} = 65536$），支持加、减、乘、除以及比较操作 (`gte`, `lte`)。
*   `std::utils`：范围约束工具 (`range_check` 及 `assert_in_range`)。

---

## 语法示例

开发者只需声明计算逻辑与隐私属性，编译器将自动处理低层编译与电路结构优化。

```dcl
use std::crypto;
use std::utils;

module AgeVerification

type Credential = {
    age:     Field,        // 私有：实际年龄
    id_hash: Field,        // 私有：身份信息哈希
}

circuit verify_adult(
    private cred: Credential,   // 私有输入（Prover 可见，不暴露给 Verifier）
    public  threshold: Field,   // 公开输入（双方可见）
) -> bool {

    // 范围断言
    assert cred.age >= threshold;

    // 使用零知识证明友好的 Poseidon 哈希
    let computed_hash = crypto::poseidon(cred.age, cred.id_hash);

    // 返回等价性验证结果
    return computed_hash == cred.id_hash;
}
```

---

## 快速开始 (Phase 0 PoC)

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

## 开发路线图

*   **Phase 0：概念验证 (PoC)** (当前阶段)：基于 Python + JAX 搭建最小化可微优化器。在三大 Benchmark 电路上证明相比静态启发式优化器（如 Circom `--O2`）具有更少的约束。
*   **Phase 1：语言前端**：完成 DCL v0.1 的语法规范制定，并使用 Rust 实现 Lexer, Parser, 类型检查器（Field 有限域类型及隐私修饰符）和 AST → DCIR 降低。
*   **Phase 2：编译器核心**：在 Rust 中实现完整的 DCIR 计算图、轻量级自动微分引擎、Gumbel-Softmax 离散化Pass，以及基于 SMT 求解器 (Z3) 的等价性验证。
*   **Phase 3：生态与扩展**：实现多后端生成（Arkworks R1CS、Noir 兼容的 ACIR、TFHE-rs），推出完整的 CLI 工具以及 VS Code 开发插件。

---

## 开源许可

本项目采用 MIT 开源许可证。详见 [LICENSE] 文件。

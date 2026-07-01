# 🔮 DCL — Differentiable Cryptographic Language

## 一门将自动微分嵌入编译器管线、用梯度下降优化密码学电路的新型编程语言

---

## 一、愿景与定位

### 1.1 一句话定义

**DCL 是一门面向隐私计算（ZKP / FHE）的编译型编程语言，其编译器的中间表示 (IR) 具有完全可微性（Fully Differentiable），能够在编译期通过梯度优化自动搜索约束数量最少、噪声增长最慢的密码学电路结构。**

### 1.2 解决什么痛点？

```
现有工作流（痛苦）:
┌──────────┐     手动优化     ┌────────────┐     静态编译     ┌──────────┐
│ 高层业务  │ ──────────────→ │ 电路逻辑    │ ──────────────→ │ R1CS/AIR │
│ 逻辑代码  │   需要密码学    │ (Circom等)  │  启发式规则     │ 约束系统  │
└──────────┘   专家手动调优   └────────────┘   无法全局最优   └──────────┘

DCL 工作流（目标）:
┌──────────┐     自动编译     ┌────────────┐   梯度下降优化   ┌──────────┐
│ 高层业务  │ ──────────────→ │ 可微 IR     │ ──────────────→ │ 最优电路 │
│ 逻辑代码  │   无需密码学    │ (DCIR)      │   自动搜索      │ 约束系统  │
└──────────┘   领域知识       └────────────┘   全局最优结构   └──────────┘
```

### 1.3 与现有语言的关系

| 维度 | Circom | Noir | Leo | **DCL (本项目)** |
| :--- | :--- | :--- | :--- | :--- |
| **抽象层级** | 低层（手动约束） | 中层（Rust-like） | 中层（应用层） | **高层（声明式数学）** |
| **优化方式** | 手动 Gadget | 静态编译器 Pass | 静态编译器 Pass | **可微编译器 + 梯度搜索** |
| **后端** | R1CS (Groth16) | ACIR (多后端) | snarkVM | **ACIR + R1CS + FHE (多后端)** |
| **核心创新** | 开创性 DSL | 后端无关 | 形式化验证 | **编译期可微优化** |

---

## 二、语言设计（前端）

### 2.1 设计哲学

> **"Write math, compile proofs."** —— 写数学，编译证明。

开发者只需要用接近数学论文的声明式语法描述计算逻辑和隐私约束，编译器负责将其转化为最优电路。

### 2.2 核心语法示例

```dcl
// ============================================================
// DCL 语法示例：一个隐私的年龄验证电路
// 证明"我的年龄 >= 18"，但不暴露实际年龄
// ============================================================

// --- 模块声明 ---
module AgeVerification

// --- 类型系统 ---
// DCL 使用有限域类型（Field）作为核心算术类型
// 支持 ADT (代数数据类型) 和模式匹配

type Credential = {
    age:     Field,        // 私有：实际年龄
    id_hash: Field,        // 私有：身份哈希
}

// --- 入口函数：circuit 关键字声明这是一个可证明电路 ---
circuit verify_adult(
    private cred: Credential,   // 私有输入（Prover 知道）
    public  threshold: Field,   // 公开输入（Verifier 也知道）
) -> bool {

    // 声明式约束：编译器自动将其转化为 R1CS 约束
    // assert 是一级约束原语，不是运行时断言
    assert cred.age >= threshold

    // 使用 ZK-friendly 哈希（编译器自动选择最优原语）
    let computed_hash = poseidon(cred.age, cred.id_hash)

    // 返回验证结果
    return computed_hash == cred.id_hash
}
```

### 2.3 关键语法特性

#### 2.3.1 隐私修饰符（Privacy Qualifiers）

```dcl
// private: 仅 Prover 可见，在证明中被隐藏
// public:  Verifier 也可见
// shared:  多方计算 (MPC) 场景下的共享秘密

circuit transfer(
    private sender_balance: Field,
    private amount: Field,
    public  receiver_addr: Field,
    shared  escrow_key: Field,       // MPC 扩展
) -> Field { ... }
```

#### 2.3.2 优化提示注解（Optimization Hints）

```dcl
// 开发者可以给编译器提供优化提示，但不是强制的
// 编译器的可微优化器可能会忽略它们（如果找到了更好的方案）

@hint(strategy = "boolean_decomposition")  // 建议使用布尔分解
@hint(max_constraints = 500)               // 建议约束上限
circuit range_proof(private x: Field, public bits: u32) -> bool {
    assert x < 2^bits
    return true
}
```

#### 2.3.3 可组合电路（Composable Circuits）

```dcl
// 电路像函数一样可以组合调用
// 编译器在 IR 层面进行全局内联和跨电路优化

circuit merkle_proof(
    private leaf: Field,
    private path: [Field; DEPTH],
    public  root: Field,
) -> bool {
    let mut current = leaf
    for sibling in path {
        // poseidon_merge 本身也是一个子电路
        current = poseidon_merge(current, sibling)
    }
    assert current == root
    return true
}
```

#### 2.3.4 代数数据类型 + 模式匹配

```dcl
// 枚举类型，编译为电路中的条件分支
enum AssetType {
    Token(Field),         // 同质化代币
    NFT(Field, Field),    // 非同质化（id, metadata_hash）
}

circuit verify_asset(private asset: AssetType) -> Field {
    match asset {
        AssetType::Token(amount)    => amount,
        AssetType::NFT(id, hash)    => poseidon(id, hash),
    }
}
```

---

## 三、编译器架构（核心创新）

### 3.1 整体架构图

```
┌─────────────────────────────────────────────────────────────────────────┐
│                          DCL Compiler Pipeline                         │
│                                                                        │
│  ┌──────────┐   ┌───────────┐   ┌──────────────┐   ┌───────────────┐  │
│  │ Frontend │   │  DCIR     │   │ Differentiable│   │   Backend     │  │
│  │          │   │  (可微IR)  │   │  Optimizer    │   │   Codegen     │  │
│  │ ┌──────┐ │   │           │   │               │   │               │  │
│  │ │Lexer │ │   │ ┌───────┐ │   │ ┌───────────┐ │   │ ┌───────────┐ │  │
│  │ └──┬───┘ │   │ │ DAG   │ │   │ │Cost Func  │ │   │ │ R1CS Gen  │ │  │
│  │    │     │   │ │ Nodes │◄├───┤►│ (可微损失) │ │   │ │           │ │  │
│  │ ┌──▼───┐ │   │ └───────┘ │   │ └─────┬─────┘ │   │ ├───────────┤ │  │
│  │ │Parser│ │   │           │   │       │       │   │ │ ACIR Gen  │ │  │
│  │ └──┬───┘ │   │ ┌───────┐ │   │ ┌─────▼─────┐ │   │ │           │ │  │
│  │    │     │   │ │ Type  │ │   │ │ Gradient  │ │   │ ├───────────┤ │  │
│  │ ┌──▼───┐ │   │ │ Info  │ │   │ │ Descent   │ │   │ │ FHE Gen   │ │  │
│  │ │ AST  │─├──►│ └───────┘ │   │ │ Engine    │ │   │ │ (TFHE/BGV)│ │  │
│  │ └──────┘ │   │           │   │ └───────────┘ │   │ └───────────┘ │  │
│  └──────────┘   └───────────┘   └──────────────┘   └───────────────┘  │
│                                                                        │
│       Phase 1           Phase 2          Phase 3          Phase 4      │
│      解析 → AST      AST → 可微IR     梯度优化 IR      IR → 目标电路    │
└─────────────────────────────────────────────────────────────────────────┘
```

### 3.2 Phase 1：前端解析

| 组件 | 职责 |
| :--- | :--- |
| **Lexer** | 将 `.dcl` 源代码分词为 Token 流 |
| **Parser** | 将 Token 流构建为 AST（抽象语法树） |
| **Type Checker** | 静态类型检查 + 隐私修饰符一致性验证 |
| **Desugaring** | 将 `for` 循环展开、`match` 转为条件树等 |

> **技术选型**：前端用 **Rust** 实现，使用 `logos` 做 Lexer，`chumsky` 做 Parser。

### 3.3 Phase 2：DCIR — 可微中间表示（核心创新 ①）

DCIR（Differentiable Cryptographic Intermediate Representation）是本语言的核心数据结构。它是一个**可微分的有向无环计算图 (DAG)**。

#### 3.3.1 DCIR 节点类型

```rust
// DCIR 的核心节点定义（Rust 伪代码）
enum DCIRNode {
    // === 算术原语 ===
    Add(NodeId, NodeId),          // 线性运算（R1CS 中"免费"）
    Sub(NodeId, NodeId),          // 线性运算（"免费"）
    Mul(NodeId, NodeId),          // 非线性运算（R1CS 约束的主要成本！）
    Const(FieldElement),          // 常量

    // === 控制流（已展平为选择器）===
    Select(NodeId, NodeId, NodeId), // if-then-else → MUX 门

    // === 密码学原语 ===
    PoseidonHash(Vec<NodeId>),    // ZK-friendly 哈希
    PedersenCommit(NodeId, NodeId), // Pedersen 承诺

    // === 约束 ===
    AssertEq(NodeId, NodeId),     // 等式约束
    AssertBool(NodeId),           // 布尔约束 (b * (1-b) = 0)
    RangeCheck(NodeId, u32),      // 范围约束

    // === 结构化 ===
    SubCircuit(CircuitId, Vec<NodeId>), // 子电路调用
}
```

#### 3.3.2 可微性：每个节点的"松弛参数"

**这是 DCL 的核心发明。** DCIR 中的每个节点不仅携带计算语义，还携带一组**可学习的连续松弛参数 (learnable relaxation parameters)**，用于在编译期被梯度优化器调整。

```rust
struct DCIRNodeMeta {
    node:        DCIRNode,
    // ===== 可微参数（编译期由优化器调整）=====
    // α: 结构选择权重。例如：一个 RangeCheck 可以用布尔分解(α→0)或查找表(α→1)实现
    alpha:       Vec<f64>,  // 软选择参数（Gumbel-Softmax）
    // β: 精度/噪声平衡参数。用于 FHE 场景下的噪声预算分配
    beta:        f64,
    // γ: 内联/折叠决策参数。控制子电路是否内联展开
    gamma:       f64,
}
```

### 3.4 Phase 3：可微优化器引擎（核心创新 ②）

这是整个编译器中最具突破性的模块。它将传统编译器的"启发式 Pass"替换为一个**端到端可微的损失函数 + 梯度下降循环**。

#### 3.4.1 损失函数设计

编译器的优化目标被形式化为一个**可微损失函数 L**：

```
L_total = w₁ · L_constraints + w₂ · L_noise + w₃ · L_depth + w₄ · L_correctness

其中：
  L_constraints = Σ (每个 Mul 节点的 cost)       ← 最小化 R1CS 约束总数
  L_noise       = max(噪声预算消耗链)            ← 最小化 FHE 噪声增长
  L_depth       = DAG 的最长路径长度              ← 最小化证明深度（影响验证时间）
  L_correctness = Σ (违反等价性约束的惩罚)        ← 保证数学等价性

  w₁, w₂, w₃, w₄ 是可配置的超参数
```

#### 3.4.2 优化循环

```
算法：DCL 编译期可微优化

输入：DCIR 图 G（包含所有节点及其松弛参数 α, β, γ）
输出：优化后的 DCIR 图 G*

1. 初始化所有松弛参数为均匀分布 / 启发式默认值
2. FOR epoch = 1 TO max_epochs:
   a. 前向传播：
      - 遍历 DCIR 图，计算每个节点的"软实现"成本
      - 使用 Gumbel-Softmax 使离散选择（如"用布尔分解 vs 查找表"）可微
      - 累计总损失 L_total
   b. 反向传播：
      - 计算 ∂L_total / ∂α, ∂L_total / ∂β, ∂L_total / ∂γ
   c. 参数更新：
      - α ← α - lr · ∂L/∂α  (使用 Adam 优化器)
      - β ← β - lr · ∂L/∂β
      - γ ← γ - lr · ∂L/∂γ
   d. 等价性验证（每 N 步执行一次）：
      - 在随机输入上对比优化前后的电路输出
      - 若不等价，增大 L_correctness 的权重 w₄
3. 离散化：
   - 将所有连续松弛参数取 argmax，得到离散的实现选择
4. RETURN 优化后的 G*
```

#### 3.4.3 Gumbel-Softmax 技巧：使离散选择可微

举例：一个 `RangeCheck(x, 8)` 节点（证明 x 是 8-bit 整数）有 3 种实现策略：

| 策略 | R1CS 约束数 | 电路深度 | FHE 噪声 |
| :--- | :--- | :--- | :--- |
| A: 逐位布尔分解 | 8 个 | 深度 1 | 低 |
| B: 查找表 (Lookup Table) | 1 个 | 深度 log(n) | 中 |
| C: 多项式近似 | ~3 个 | 深度 3 | 高 |

传统编译器只能用固定规则选择（如"总是用策略 A"）。DCL 的编译器通过 Gumbel-Softmax 让 `α = [α_A, α_B, α_C]` 在训练中**软选择**最优策略：

```
实际成本 = softmax(α_A/τ) * cost_A + softmax(α_B/τ) * cost_B + softmax(α_C/τ) * cost_C

其中 τ (温度) 逐渐退火至 0，最终 argmax 硬选择出最优方案。
```

### 3.5 Phase 4：后端代码生成

优化后的 DCIR 被降低（Lowered）为具体的目标格式：

```
                          ┌────────────────┐
                          │  Optimized     │
                          │  DCIR Graph    │
                          └───────┬────────┘
                                  │
                    ┌─────────────┼─────────────┐
                    │             │             │
              ┌─────▼────┐ ┌─────▼────┐ ┌─────▼─────┐
              │  R1CS    │ │  ACIR    │ │  FHE IR   │
              │ Backend  │ │ Backend  │ │ Backend   │
              │(Groth16) │ │(Barret.) │ │(TFHE/BGV) │
              └─────┬────┘ └─────┬────┘ └─────┬─────┘
                    │             │             │
              ┌─────▼────┐ ┌─────▼────┐ ┌─────▼─────┐
              │ .r1cs    │ │ .acir    │ │ .fhe.bin  │
              │ 约束文件  │ │ 电路文件  │ │ 密文程序  │
              └──────────┘ └──────────┘ └───────────┘
```

---

## 四、基于 MLIR 的实现方案

### 4.1 MLIR Dialect 设计

DCL 编译器基于 **LLVM/MLIR** 基础设施构建，定义两个核心 Dialect：

#### Dialect 1: `dcl` — 高层语义 Dialect

```mlir
// MLIR 中的 DCL 高层表示示例
func.func @verify_adult(%cred_age: !dcl.field, %threshold: !dcl.field) -> i1 {
    %is_adult = dcl.assert_gte %cred_age, %threshold : !dcl.field
    %hash     = dcl.poseidon [%cred_age] : !dcl.field -> !dcl.field
    %result   = dcl.assert_eq %hash, %cred_age : !dcl.field
    return %result : i1
}
```

#### Dialect 2: `dcir` — 可微中间表示 Dialect

```mlir
// 降低后的 DCIR 表示，每个 op 携带松弛参数
func.func @verify_adult_dcir(%arg0: !dcir.wire, %arg1: !dcir.wire) -> !dcir.wire {
    // alpha 参数控制 range_check 的实现策略选择
    %0 = dcir.range_check %arg0 {
        bits = 8,
        alpha = dense<[0.33, 0.33, 0.34]> : tensor<3xf64>,  // 可微参数！
        strategies = ["boolean", "lookup", "polynomial"]
    } : !dcir.wire -> !dcir.wire

    %1 = dcir.mul %arg0, %arg1 {
        beta = 0.5 : f64   // 噪声预算分配参数
    } : !dcir.wire, !dcir.wire -> !dcir.wire

    return %1 : !dcir.wire
}
```

### 4.2 Pass Pipeline（编译流水线）

```
dcl.module
  │
  ├── [Pass 1] dcl-to-dcir          # 高层 Dialect → 可微 IR Dialect
  ├── [Pass 2] dcir-canonicalize     # 标准化（常量折叠、死代码消除）
  ├── [Pass 3] dcir-inline           # 子电路内联（受 γ 参数控制）
  │
  ├── [Pass 4] dcir-diff-optimize    # ★ 核心：可微优化循环 ★
  │     ├── Forward: 计算 L_total
  │     ├── Backward: 反向传播梯度
  │     ├── Update: Adam 更新 α, β, γ
  │     └── Discretize: Gumbel-Softmax 退火 → 离散选择
  │
  ├── [Pass 5] dcir-verify           # 等价性验证（随机测试 + SMT）
  ├── [Pass 6] dcir-to-r1cs          # DCIR → R1CS 约束系统
  │         OR dcir-to-acir          # DCIR → ACIR (Noir 兼容)
  │         OR dcir-to-fhe           # DCIR → FHE 密文程序
  │
  └── Output: .r1cs / .acir / .fhe.bin
```

---

## 五、项目结构与技术栈

### 5.1 目录结构

```
dcl/
├── crates/
│   ├── dcl-frontend/           # Rust: Lexer + Parser + AST
│   │   ├── src/
│   │   │   ├── lexer.rs
│   │   │   ├── parser.rs
│   │   │   ├── ast.rs
│   │   │   └── typechecker.rs
│   │   └── Cargo.toml
│   │
│   ├── dcl-ir/                 # Rust: DCIR 数据结构 + 图操作
│   │   ├── src/
│   │   │   ├── graph.rs        # DAG 图结构
│   │   │   ├── nodes.rs        # DCIRNode 定义
│   │   │   ├── meta.rs         # 松弛参数 (α, β, γ)
│   │   │   └── cost.rs         # 各策略的成本模型
│   │   └── Cargo.toml
│   │
│   ├── dcl-optimizer/          # Rust + Python: 可微优化引擎
│   │   ├── src/
│   │   │   ├── loss.rs         # 损失函数 L_total
│   │   │   ├── autograd.rs     # 自动微分引擎（反向传播）
│   │   │   ├── gumbel.rs       # Gumbel-Softmax 实现
│   │   │   ├── adam.rs         # Adam 优化器
│   │   │   └── verifier.rs     # 等价性验证
│   │   └── Cargo.toml
│   │
│   ├── dcl-backend-r1cs/       # Rust: R1CS 代码生成
│   │   └── src/
│   │       ├── codegen.rs
│   │       └── groth16.rs
│   │
│   ├── dcl-backend-acir/       # Rust: ACIR 代码生成 (Noir 兼容)
│   │   └── src/
│   │       └── codegen.rs
│   │
│   ├── dcl-backend-fhe/        # Rust: FHE 代码生成 (TFHE)
│   │   └── src/
│   │       ├── codegen.rs
│   │       └── noise_model.rs  # 噪声预算追踪
│   │
│   └── dcl-cli/                # 命令行工具
│       └── src/
│           └── main.rs         # dcl build / dcl prove / dcl verify
│
├── mlir/                       # C++: MLIR Dialect 定义
│   ├── include/
│   │   ├── DCL/                # dcl dialect ODS (TableGen)
│   │   └── DCIR/               # dcir dialect ODS
│   └── lib/
│       ├── DCL/                # dcl dialect 实现
│       ├── DCIR/               # dcir dialect 实现
│       └── Transforms/         # MLIR Pass 实现
│
├── stdlib/                     # DCL 标准库
│   ├── crypto/
│   │   ├── poseidon.dcl        # Poseidon 哈希
│   │   ├── pedersen.dcl        # Pedersen 承诺
│   │   └── merkle.dcl          # Merkle 树
│   ├── math/
│   │   ├── field.dcl           # 有限域算术
│   │   └── polynomial.dcl      # 多项式运算
│   └── utils/
│       └── range.dcl           # 范围证明
│
├── examples/                   # 示例程序
│   ├── age_verify.dcl
│   ├── private_vote.dcl
│   └── zkml_inference.dcl      # ZK 机器学习推理
│
├── tests/                      # 测试
│   ├── correctness/            # 电路等价性测试
│   ├── optimization/           # 优化效果基准测试
│   └── e2e/                    # 端到端证明/验证测试
│
├── docs/                       # 文档
│   ├── language_spec.md        # 语言规范
│   ├── compiler_design.md      # 编译器设计文档
│   └── tutorial.md             # 教程
│
├── Cargo.toml                  # Rust workspace
├── CMakeLists.txt              # MLIR 构建
└── README.md
```

### 5.2 技术栈选型

| 模块 | 技术 | 理由 |
| :--- | :--- | :--- |
| **编译器前端** | Rust (`logos` + `chumsky`) | 高性能、内存安全、与密码学生态一致 |
| **IR 与优化器** | Rust + 自研 autograd | 需要精确控制内存布局和计算精度 |
| **MLIR 集成** | C++ (LLVM/MLIR) | 复用 MLIR 基础设施（TableGen, PassManager） |
| **R1CS 后端** | Rust (`ark-relations`) | arkworks 是最成熟的 Rust ZKP 库 |
| **ACIR 后端** | Rust (`acvm`) | 复用 Noir 的 ACIR 虚拟机 |
| **FHE 后端** | Rust (`tfhe-rs`) + Go (`Lattigo`) | TFHE-rs 性能最优；Lattigo 生态完善 |
| **等价性验证** | Z3 SMT Solver (via `z3-sys`) | 工业级 SMT 求解器 |
| **CLI 工具** | Rust (`clap`) | 统一的命令行体验 |

---

## 六、关键算法细节

### 6.1 自动微分引擎（`dcl-optimizer/autograd.rs`）

DCL 的 autograd 引擎不是 PyTorch 那种通用张量 AD，而是一个**专为 DAG 计算图优化的轻量级反向模式 AD**：

```rust
// 伪代码：DCIR 图上的反向传播
fn backward(graph: &DCIRGraph, loss: f64) -> Gradients {
    // 拓扑排序（反向）
    let topo_order = graph.topological_sort().reverse();

    let mut grads = HashMap::new();
    grads.insert(loss_node, 1.0);  // dL/dL = 1

    for node in topo_order {
        let grad = grads[&node];
        match &node.op {
            // Mul 门的梯度：∂(a*b)/∂a = b, ∂(a*b)/∂b = a
            Mul(a, b) => {
                grads[a] += grad * graph.value(b);
                grads[b] += grad * graph.value(a);
            },
            // Gumbel-Softmax 的梯度（对 α 可微）
            Select(alpha, options) => {
                let softmax_grad = gumbel_softmax_backward(alpha, tau);
                for (i, opt) in options.iter().enumerate() {
                    grads[&alpha[i]] += grad * softmax_grad[i];
                }
            },
            // ... 其他节点的梯度规则
        }
    }
    grads
}
```

### 6.2 噪声预算追踪（FHE 专用）

```rust
// FHE 后端的噪声模型
struct NoiseTracker {
    noise_budget: f64,   // 初始噪声预算
    consumed: f64,       // 已消耗噪声
}

impl NoiseTracker {
    // 加法：噪声线性增长
    fn add_noise(&mut self, a: f64, b: f64) -> f64 {
        self.consumed += (a + b);
        a + b
    }

    // 乘法：噪声平方级增长（这是 FHE 的主要瓶颈！）
    fn mul_noise(&mut self, a: f64, b: f64) -> f64 {
        let new_noise = a * b + a + b;  // 简化的噪声模型
        self.consumed += new_noise;
        new_noise
    }

    // Bootstrapping：重置噪声（但代价极高）
    fn bootstrap(&mut self) -> f64 {
        self.consumed = BOOTSTRAP_BASE_NOISE;
        BOOTSTRAP_BASE_NOISE
    }

    // 损失函数组件：噪声超出预算的惩罚
    fn noise_loss(&self) -> f64 {
        relu(self.consumed - self.noise_budget)
    }
}
```

### 6.3 等价性验证策略

编译器必须保证优化后的电路与原始语义**数学等价**。DCL 使用三级验证策略：

| 级别 | 方法 | 时机 | 强度 |
| :--- | :--- | :--- | :--- |
| **L1: 随机测试** | 生成随机有限域元素，对比优化前后输出 | 每个优化 epoch | 快速但非完备 |
| **L2: 符号执行** | 用 Z3 SMT 求解器验证两个电路的符号等价性 | 每 N 个 epoch | 中等，可覆盖边界 |
| **L3: 形式化证明** | 输出 Lean4/Coq 证明义务，手动或自动证明 | 最终发布前 | 完备，但需要人工 |

---

## 七、开发路线图

### Phase 0：验证概念 (PoC) — 预计 3 个月

> **目标**：证明"可微编译器优化密码学电路"这一核心假设是可行的。

- [ ] 用 Python (JAX) 实现一个最小化的可微 R1CS 优化器
- [ ] 在 3 个 benchmark 电路上验证（Poseidon Hash, Range Proof, Merkle Tree）
- [ ] 对比优化前后的约束数量，证明优于 Circom `--O2`
- [ ] 发布技术报告 / arXiv 论文

### Phase 1：语言前端 — 预计 4 个月

- [ ] 设计并冻结 DCL v0.1 语法规范
- [ ] 实现 Lexer + Parser + AST (Rust)
- [ ] 实现类型检查器（Field 类型 + 隐私修饰符）
- [ ] 实现 AST → DCIR 降低

### Phase 2：编译器核心 — 预计 6 个月

- [ ] 实现 DCIR 图结构及松弛参数
- [ ] 实现自研 autograd 引擎
- [ ] 实现 Gumbel-Softmax 离散化
- [ ] 实现损失函数 L_total 及 Adam 优化器
- [ ] 实现 L1 + L2 等价性验证
- [ ] R1CS 后端代码生成

### Phase 3：生态与扩展 — 预计 6 个月

- [ ] ACIR 后端（兼容 Noir 生态）
- [ ] FHE 后端（TFHE-rs 集成）
- [ ] 标准库（poseidon, pedersen, merkle, range_proof）
- [ ] `dcl` CLI 工具（build, prove, verify, benchmark）
- [ ] VS Code 插件（语法高亮 + LSP）
- [ ] MLIR 集成（可选，用于复用 MLIR 优化 passes）

### Phase 4：社区与论文 — 持续

- [ ] 撰写 PLDI/POPL 级别学术论文
- [ ] 开源并建立社区
- [ ] Benchmark 套件：与 Circom, Noir, Leo 的性能对比

---

## 八、风险与缓解

| 风险 | 严重度 | 缓解措施 |
| :--- | :--- | :--- |
| 可微优化可能陷入局部最优 | 🟡 中 | 多起点随机初始化 + 模拟退火；同时保留传统启发式 Pass 作为 baseline |
| 等价性验证无法保证 100% 正确 | 🔴 高 | L3 形式化证明；对安全关键场景强制要求 L3 验证 |
| Gumbel-Softmax 退火后选择震荡 | 🟡 中 | 使用 Straight-Through Estimator (STE) 作为备选梯度估计器 |
| 编译时间过长（梯度循环） | 🟡 中 | 增量编译；缓存已优化的子电路；限制 max_epochs |
| 生态冷启动问题 | 🟡 中 | Phase 3 优先实现 ACIR 后端，直接复用 Noir 生态的证明器和工具 |

---

## 九、理论基础与参考文献

### 核心理论
1. **R1CS 与算术电路**：Rank-1 Constraint Systems 是 ZKP 的数学基础。在 R1CS 中，加法"免费"，乘法产生约束。DCL 的损失函数直接以 Mul 节点数量为优化目标。
2. **Gumbel-Softmax**：Jang et al., 2017. "Categorical Reparameterization with Gumbel-Softmax"。使离散选择可微。
3. **MLIR 多级 IR**：Chris Lattner et al., 2021. "MLIR: Scaling Compiler Infrastructure for Domain Specific Computation"。
4. **FHE 噪声模型**：Chillotti et al., 2020. "TFHE: Fast Fully Homomorphic Encryption over the Torus"。

### 现有工具（竞争/互补关系）
5. **Circom** — 低层 ZKP DSL，手动约束管理
6. **Noir (Aztec)** — 后端无关的 ZKP 语言，ACIR 中间表示
7. **Leo (Aleo)** — 应用层 ZKP 语言，形式化验证
8. **Concrete (Zama)** — FHE 编译器，基于 MLIR
9. **HEIR (Google)** — FHE 中间表示，基于 MLIR
10. **arkworks** — Rust ZKP 库生态

---

> [!TIP]
> **建议的第一步**：在 `/Users/liuyukai/CREATE/auv` 目录下创建 `dcl-poc/` 子项目，用 Python + JAX 实现 Phase 0 的 PoC。先在一个 Poseidon Hash 电路上验证"梯度下降能否找到比 Circom `--O2` 更少约束的电路结构"。

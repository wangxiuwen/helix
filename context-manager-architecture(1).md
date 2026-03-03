# 三层上下文管理系统 — 架构与实现文档

> 灵感来源: Google Gemini CLI (Antigravity) 的 `ChatCompressionService` + `ToolOutputMaskingService`  
> 实现文件: `src/agent/context-manager.js`  
> 集成点: `src/agent/core.js` + `src/agent/team-orchestrator.js`

---

## 一、问题背景

### 为什么 Agent 会卡死？

在多轮工具调用场景中(比如连续读取多个大文件)，对话上下文会迅速膨胀：

```
Round 1: user(100) + assistant(50) + tool_result(20,000)    = 20,150 tokens
Round 5: 累积 ≈ 80,000 tokens
Round 10: 累积 ≈ 200,000+ tokens  ← 超过 131K 上下文窗口！
```

**结果**: LLM API 返回 400 错误，或请求超时 → Agent 挂住。

### 旧方案的问题

旧方案只有一层防御：`MAX_HISTORY = 40` (按消息条数裁剪)。但 40 条消息如果每条含大量工具输出，token 总量仍可能超过 100K。

---

## 二、整体架构

```
┌─────────────────────────────────────────────────────────────┐
│                    用户/Agent 发送请求                         │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           ▼
┌──────────────────────────────────────────────────────────────┐
│  第一层: Tool Output Masking (工具输出遮蔽)                    │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ • 反向扫描所有 tool 消息                                 │  │
│  │ • 最新 30K tokens 的工具输出 → 保护，不碰               │  │
│  │ • 超出保护窗口的大输出 (>8KB) → 截断为 head+tail         │  │
│  │ • 批量阈值: 可修剪总量 >15K tokens 才执行               │  │
│  │                                                        │  │
│  │ 效果: 431K tokens → 60K tokens (↓86%)                  │  │
│  └────────────────────────────────────────────────────────┘  │
│  触发时机: 每次 _sanitizeMessages() 调用时                    │
│  执行方式: 纯计算,无额外 API 调用,零延迟                      │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           ▼
┌──────────────────────────────────────────────────────────────┐
│  第二层: Chat Compression (对话压缩)                          │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ • 检测: 当前 tokens > 模型上下文 × 45%                  │  │
│  │ • 分割: 前 65% 的历史 → 交给 LLM 摘要                  │  │
│  │        后 35% 的历史 → 原样保留                         │  │
│  │ • 摘要: LLM 生成 <state_snapshot>                       │  │
│  │ • 膨胀检测: 压缩后 token ≥ 压缩前 → 放弃               │  │
│  │ • 新历史: [snapshot] + [assistant ack] + [保留部分]      │  │
│  │                                                        │  │
│  │ 效果: 保留关键上下文信息,不丢失技术细节                   │  │
│  └────────────────────────────────────────────────────────┘  │
│  触发时机: 每 5-8 轮工具调用检查一次                           │
│  执行方式: 调用 LLM API 一次 (摘要生成)                       │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           ▼
┌──────────────────────────────────────────────────────────────┐
│  第三层: Overflow Prevention (溢出预防)                        │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ • 计算: system_prompt + messages 的总 token             │  │
│  │ • 硬上限: 模型上下文窗口 × 85%                          │  │
│  │ • 安全 → 正常发送请求                                   │  │
│  │ • 不安全 → 前端告警 🚨 + 触发紧急压缩                   │  │
│  │ • 紧急压缩失败 → 只保留最新 30% 消息 (Emergency Trim)   │  │
│  │                                                        │  │
│  │ 效果: 绝不让请求因 token 超限而崩溃                      │  │
│  └────────────────────────────────────────────────────────┘  │
│  触发时机: 每轮请求前必检                                     │
│  执行方式: 纯计算 + 条件触发压缩                              │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           ▼
┌──────────────────────────────────────────────────────────────┐
│  安全网: Message Count Trimming (消息条数裁剪)                │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ • MAX_HISTORY = 60 (从旧的 40 提升)                     │  │
│  │ • 超过 60 条 → 保留第一条 user + 最新 59 条             │  │
│  │ • 这是最后一道防线,正常不会触发                          │  │
│  └────────────────────────────────────────────────────────┘  │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           ▼
┌──────────────────────────────────────────────────────────────┐
│  原有逻辑: Sanitize (清理孤立工具调用 + 图片去重)              │
│  → 确保 tool_call ↔ tool_result 配对完整                     │
│  → 只保留最新一张图片,旧图片替换为占位符                      │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           ▼
                    发送给 LLM API ✅

```

---

## 三、配置参数一览

### Token 估算

```javascript
const CHARS_PER_TOKEN = 3;  // 中文≈2, 英文≈4, 取中值3
```

### 模型上下文窗口

| 模型 | 上下文 (tokens) |
|------|----------------|
| qwen3.5-plus / qwen3-coder-next 等 | 131,072 |
| claude-opus-4 / claude-sonnet-4 | 200,000 |
| gemini-2.5-pro / flash | 1,048,576 |
| gpt-4o / o3 | 128,000 |
| 默认值 | 131,072 |

### Layer 1: Tool Output Masking

| 参数 | 值 | 说明 |
|------|-----|------|
| `TOOL_PROTECTION_THRESHOLD` | 30,000 tokens | 最新工具输出的保护窗口 |
| `MIN_PRUNABLE_THRESHOLD` | 15,000 tokens | 可修剪累积量 > 此值才执行 |
| `MAX_TOOL_RESULT_CHARS` | 8,000 chars | 单个工具结果超此长度才可修剪 |
| `PREVIEW_HEAD_CHARS` | 500 chars | 截断后保留的开头 |
| `PREVIEW_TAIL_CHARS` | 500 chars | 截断后保留的结尾 |

### Layer 2: Chat Compression

| 参数 | 值 | 说明 |
|------|-----|------|
| `COMPRESSION_TOKEN_THRESHOLD` | 0.45 (45%) | 超过模型上下文的 45% 触发 |
| `COMPRESSION_PRESERVE_RATIO` | 0.35 (35%) | 保留最新 35% 的历史不压缩 |

### Layer 3: Overflow Prevention

| 参数 | 值 | 说明 |
|------|-----|------|
| `OVERFLOW_SAFETY_MARGIN` | 0.85 (85%) | 上下文窗口的 85% 为硬上限 |

---

## 四、各层详细实现

### 4.1 第一层: Tool Output Masking

**核心函数**: `maskToolOutputs(messages)`

**算法**: 反向保护窗口 + 批量触发

```
输入: [user, assistant+tool_calls, tool_result(50KB), user, ..., tool_result(2KB)]
                                    ↑ 旧的大输出                         ↑ 新的小输出

处理流程:
  1. 从最新消息倒着扫描 → 累计 tool 消息的 token
  2. 累计 < 30K tokens → 属于"保护窗口",不碰
  3. 累计 ≥ 30K tokens → 之后的大工具输出标记为"可修剪"
  4. 可修剪总量 < 15K tokens → 不值得修剪,跳过
  5. 可修剪总量 ≥ 15K tokens → 批量执行修剪

修剪方式:
  原始: "import express from 'express';\nimport...(20000字)...\nexport default app;"
  修剪后:
    "[Tool output truncated — original: 450 lines, ~20KB]
     import express from 'express';\nimport dotenv from...(前500字)...
     ...[430 lines omitted]...
     ...(后500字)...export default app;"
```

**设计意图**:
- **保护窗口**: 最新的工具输出通常是 Agent 正在使用的,不能碰
- **批量阈值**: 避免频繁小修剪的开销
- **Head+Tail 保留**: 文件开头(imports/headers)和结尾(exports)通常是最有信息量的

---

### 4.2 第二层: Chat Compression

**核心函数**: `compressChat(messages, provider, modelId)`

**算法**: LLM 摘要 + 分割保留 + 膨胀检测

```
触发条件: currentTokens > modelLimit × 45%
  例: qwen3-coder-next → 131K × 0.45 = 58,982 tokens

分割策略:
  ┌──────────────────────────────────────────────┐
  │ 前 65% 的对话历史 (按字符量)                   │
  │ → 交给 LLM 摘要成 <state_snapshot>            │
  └──────────────────────────────────────────────┘
  ┌──────────────────────────────────────────────┐
  │ 后 35% 的对话历史                              │
  │ → 原样保留,不动                               │
  └──────────────────────────────────────────────┘

分割约束:
  - 只在 user 消息边界分割
  - 避免打断 [assistant+tool_calls] ↔ [tool_result] 的配对关系

压缩 Prompt (关键):
  "You are a context compression assistant..."
  Rules:
  1. 保留所有: 文件路径、函数名、变量名、错误信息、工具结果、技术决策
  2. 保留用户偏好、约束、显式需求
  3. 记录使用了哪些工具及关键结果
  4. 追踪进行中任务的当前状态
  5. 简洁但不丢失可执行信息
  6. 与对话同语言输出
  7. 格式: <state_snapshot>...</state_snapshot>

膨胀检测:
  if (afterTokens >= beforeTokens) → 放弃压缩,使用原始消息

新历史结构:
  [
    { role: 'user', content: '[Context Snapshot — compressed from N messages]\n\n<state_snapshot>...' },
    { role: 'assistant', content: '明白，我已掌握之前的上下文。请继续。' },
    ...保留的后 35%...
  ]
```

**设计意图**:
- **45% 阈值**: 留足空间给 system prompt + 新消息 + 模型输出
- **35% 保留**: 最新的对话对当前任务最重要,不能被摘要替代  
- **分割边界**: 在 user 消息处分割,确保 tool_call/tool_result 对完整  
- **膨胀检测**: LLM 有时会生成比原文还长的"摘要",要检测并放弃

---

### 4.3 第三层: Overflow Prevention

**核心函数**: `checkOverflow(messages, systemPrompt, modelId)`

**算法**: 预估 + 告警 + 紧急裁剪

```
计算:
  totalTokens = estimateTokens(systemPrompt) + estimateMessagesTokens(messages)
  hardLimit = modelContextLimit × 85%
  usage = totalTokens / modelContextLimit

判断:
  totalTokens < hardLimit → safe: true  ✅
  totalTokens ≥ hardLimit → safe: false ❌

前端展示:
  每轮循环都会显示:
  "[Loop 3] Messages: 25 | Tokens: ~45000 | Context: 34%"

  溢出时:
  "🚨 上下文窗口即将溢出 (92%)。建议开始新对话。正在尝试紧急压缩..."

紧急裁剪 (最后手段):
  1. 尝试强制 compressChat()
  2. 如仍不安全 → 只保留最新 30% 消息
  3. 在开头插入: "[Earlier context was truncated due to context window overflow]"
```

**设计意图**:
- **85% 硬上限**: 留 15% 空间给 system prompt 和模型的输出 buffer
- **实时显示**: 用户能看到上下文使用率,知道何时该开新对话
- **渐进降级**: 先尝试压缩 → 再紧急裁剪,避免直接崩溃

---

## 五、集成点

### 5.1 在 `core.js` 中的集成

```javascript
// 导入
import { maskToolOutputs, compressChat, checkOverflow, estimateMessagesTokens } from './context-manager.js';

// _sanitizeMessages() — 每次调用自动执行 Layer 1
_sanitizeMessages(messages) {
    let workingMessages = maskToolOutputs(messages);  // ← Layer 1
    // ...原有的 sanitize 逻辑 (orphan tool_call 清理, 图片去重)
}

// processMessageStream() — 工具循环中执行 Layer 2 + Layer 3
while (rounds < MAX_TOOL_ROUNDS) {
    // Layer 2: 每 5 轮检查一次压缩
    if (rounds > 1 && rounds % 5 === 0) {
        const compResult = await compressChat(messages, provider, model);
        if (compResult.compressed) {
            messages.length = 0;
            messages.push(...compResult.messages);
        }
    }

    // Layer 3: 每轮检查溢出
    const overflowCheck = checkOverflow(messages, systemPrompt, model);
    onEvent({
        type: 'loop_info',
        data: `[Loop ${rounds}] Messages: ${messages.length} | Tokens: ~${...} | Context: ${overflowCheck.usagePercent}`
    });

    if (!overflowCheck.safe) {
        // 触发紧急压缩...
    }
}
```

### 5.2 在 `team-orchestrator.js` 中的集成

```javascript
// _runAgent() — 每个 Agent 的独立循环
while (rounds < MAX_ROUNDS_PER_ROLE) {
    // Layer 2: 每 8 轮检查一次 (团队模式频率低一些)
    if (rounds > 1 && rounds % 8 === 0) {
        const compResult = await compressChat(messages, provider, model);
        if (compResult.compressed) { ... }
    }
}

// _sanitizeMessages() — 同 core.js, 自动执行 Layer 1
_sanitizeMessages(messages) {
    let workingMessages = maskToolOutputs(messages);  // ← Layer 1
    // ...
}
```

---

## 六、对比: 旧方案 vs 新方案

| 维度 | 旧方案 | 新方案 |
|------|--------|--------|
| **防御层数** | 1 层 (消息条数裁剪) | 4 层 (遮蔽 → 压缩 → 溢出预防 → 条数裁剪) |
| **Token 感知** | ❌ 不计算 token | ✅ 每轮都估算和展示 |
| **工具输出处理** | 完整保留 | 旧输出截断为 head+tail |
| **历史保留** | 直接丢弃旧消息 | LLM 摘要保留关键信息 |
| **溢出预防** | ❌ 无 | ✅ 发请求前预估,超限告警 |
| **用户感知** | 无任何提示 | 实时显示 token 使用率 |
| **MAX_HISTORY** | 40 | 60 (因为有真正的 token 管理) |
| **Agent 卡死风险** | 高 | 极低 |

---

## 七、验证数据

### 模拟 15 轮大文件读取

```
=== Before masking ===
Total messages: 45
Total tokens: 431,245

  🗜️ Context: masked 14 tool outputs, saved ~370,795 tokens

=== After masking ===
Total messages: 45
Total tokens: 60,450
Tokens saved: 370,795

=== Overflow Check (before masking) ===
  Tokens: 431,251 | Limit: 131,072 | Usage: 329% | Safe: false ❌

=== Overflow Check (after masking) ===
  Tokens: 60,456 | Limit: 131,072 | Usage: 46% | Safe: true ✅
```

**结论**: 仅第一层 (Tool Output Masking) 就把 431K tokens 降到 60K (↓86%)，从 329% 溢出变为 46% 安全。

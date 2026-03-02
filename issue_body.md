## 问题描述

在设置页面配置 AI 提供商（Base URL、API Key）后，AI 对话页面仍然报错。v0.3.0 版本存在以下 Bug。

## 复现步骤

1. 在设置 → AI 提供商中添加提供商（如 CodingPlan / DashScope）
2. 填写 Base URL 和 API Key，点击保存
3. 回到 AI 对话页面，发送消息
4. 显示「请求失败: 未知错误」（实际是 API Key 未同步到后端）

## 根本原因

### Bug 1: syncAIProviderToBackend 因 __TAURI__ 检查失败而静默跳过

`src/stores/useDevOpsStore.ts` 第 7 行：

```typescript
if (typeof window === 'undefined' || !(window as any).__TAURI__) return;
```

`tauri.conf.json` 中 `withGlobalTauri` 为 `false`，导致 `window.__TAURI__` 不存在。函数直接 return，`ai_set_config` 从未被调用，API Key 不会写入 `helix_config.json`。

**建议修复：** 移除 `__TAURI__` 检查（Tauri v2 通过 `@tauri-apps/api/core` 的 `invoke` 调用，不依赖全局变量）：

```typescript
if (typeof window === 'undefined') return;
```

### Bug 2: 前端 Tauri IPC 错误显示为「未知错误」

`src/stores/useDevOpsStore.ts` 第 446 行：

```typescript
content: `❌ 请求失败: ${err.message || '未知错误'}`,
```

Tauri IPC 的 `Result<_, String>` 错误返回纯字符串，不是 Error 对象，`err.message` 为 `undefined`。

**建议修复：**

```typescript
content: `❌ 请求失败: ${typeof err === 'string' ? err : err.message || JSON.stringify(err)}`,
```

### Bug 3: useDevOpsStore.ts 存在未使用的导入导致编译失败

从 `opsTools` 导入了 `getToolsForAI`, `findTool`, `buildSkillsPrompt`, `ToolLoopDetector`, `buildAgentSkillsPrompt`, `ToolEvent` 等未使用的符号，以及未使用的 `callAI` 函数，导致 `tsc` 严格模式编译失败，无法 `npm run tauri build`。

## 环境

- OS: Windows
- Helix: v0.3.0
- Node.js: v22.21.0
- Rust: 1.93.1

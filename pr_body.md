## 修复内容

### 1. Light 模式代码块不可读 (Closes #3)
- 添加 html:not(.dark) CSS 覆盖 highlight.js github-dark 主题
- 代码块背景改为浅灰，文字改为深色
- 语法高亮 token 使用 GitHub Light 配色

### 2. AI 配置无法同步到后端 (Closes #2)
- 移除 syncAIProviderToBackend 中过时的 window.__TAURI__ 检查
- tauri.conf.json 的 withGlobalTauri: false 导致此检查永远为 false

### 3. 前端错误显示为未知错误 (Closes #2)
- Tauri IPC 返回纯字符串而非 Error 对象
- 修复 err.message 为 undefined 的问题

### 4. TypeScript 编译失败 (Closes #2)
- 移除未使用的导入和函数

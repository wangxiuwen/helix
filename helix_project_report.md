# Helix 项目报告

## 📋 项目概述

**Helix** 是一款功能强大的跨平台 AI 助理桌面应用，采用 Tauri 技术栈构建，实现了 AI Agent 与本地系统的深度集成。项目版本 0.3.0，采用 AGPL/CC-BY-NC-SA-4.0 许可证。

---

## 🏗️ 技术架构

### 前端技术栈
- **框架**：React 19 + TypeScript + Vite 7
- **UI 组件**：Ant Design 5 + DaisyUI + Tailwind CSS
- **状态管理**：Zustand
- **路由**：React Router v7
- **国际化**：i18next
- **图标库**：Lucide React + @lobehub/icons
- **动画**：Framer Motion

### 后端技术栈
- **框架**：Tauri v2 (Rust)
- **HTTP 服务**：Axum + utoipa (Swagger UI)
- **数据库**：SQLite (rusqlite)
- **AI 集成**：async-openai SDK
- **浏览器自动化**：chromiumoxide
- **任务调度**：cron + 自定义调度器

---

## 🌟 核心功能模块

### 1. AI Agent 系统
- **完整 Agent 循环**：System Prompt → User Message → AI Call → Tool Execution → Repeat
- **工具调用能力**：
  - 文件操作：读取、写入、编辑（沙箱限制）
  - 系统命令：shell_exec（支持超时控制）
  - 网络操作：web_fetch、web_search（多引擎：DuckDuckGo/Bing/Baidu）
  - 内存管理：memory_store/memory_recall
  - 进程管理：process_list、process_kill
  - 浏览器自动化：chromiumoxide 驱动

### 2. 消息平台集成
- **微信**：文件传输助手（消息收发）
- **飞书**：正则表达式解析 + 长连接事件监听
- **钉钉/企业微信**：计划中

### 3. 技能系统（Plugins）
- 动态加载/卸载技能插件
- 从 Git 仓库安装技能
- 技能清单管理与状态控制

### 4. 定时任务系统
- Cron 表达式解析与执行
- 持久化任务存储
- 调度器集成

### 5. 长期记忆系统
- 基于 SQLite 的记忆存储
- 向量搜索支持（memory_embed）
- 上下文召回机制

---

## 📁 项目结构

```
helix/
├── src/                      # React 前端
│   ├── pages/               # 页面组件
│   │   ├── AIChat.tsx       # AI 对话界面
│   │   ├── Skills.tsx       # 技能管理
│   │   ├── CronJobs.tsx     # 定时任务
│   │   └── Logs.tsx         # 系统日志
│   ├── components/          # UI 组件
│   │   ├── layout/          # 布局组件
│   │   └── common/          # 通用组件
│   ├── stores/              # Zustand 状态管理
│   ├── services/            # API 服务层
│   ├── hooks/               # 自定义 React Hooks
│   ├── locales/             # 国际化资源
│   └── utils/               # 工具函数
├── src-tauri/              # Rust 后端
│   ├── src/
│   │   ├── modules/        # 核心模块
│   │   │   ├── agent/      # Agent 核心逻辑
│   │   │   ├── ai/         # AI 提供商集成
│   │   │   ├── chat/       # 聊天模块
│   │   │   ├── infra/      # 基础设施（配置/日志/DB）
│   │   │   ├── app/        # 应用级功能（托盘/更新）
│   │   │   ├── browser/    # 浏览器引擎
│   │   │   └── evomap/     # EvoMap 模块
│   │   ├── commands/       # Tauri 命令
│   │   └── models/         # 数据模型
│   └── tauri.conf.json     # Tauri 配置
├── docs/                   # 项目文档
├── public/                 # 静态资源
└── docker/                 # Docker 部署支持
```

---

## 🔐 安全设计

1. **文件操作沙箱**：
   - 所有 `file_write`/`file_edit` 操作限制在 `~/helix_workspace` 目录
   - 路径验证机制防止目录遍历攻击
   - 严格路径规范化处理

2. **命令执行安全**：
   - Shell 命令执行支持超时控制（默认 30 秒）
   - 输出内容截断（默认 8000 字符）
   - 错误处理与异常捕获

3. **CSP 策略**：
   ```
   default-src 'self';
   img-src 'self' asset: data: https:;
   script-src 'self' 'unsafe-inline' 'unsafe-eval';
   connect-src ipc: http://ipc.localhost https: http: ws: wss:
   ```

---

## 📊 数据存储

| 数据类型 | 存储方式 | 说明 |
|---------|---------|------|
| 应用配置 | Tauri Settings | JSON 格式持久化 |
| 消息历史 | SQLite | conversation 表 |
| 技能数据 | SQLite | skills 表 |
| 定时任务 | SQLite | cron_jobs 表 |
| 长期记忆 | SQLite | memory_entries 表 |
| 系统日志 | 文件 + 内存 | tracing + 日志桥接 |

---

## 🎨 用户界面特色

1. **暗黑模式支持**：完整的深色主题适配
2. **多语言支持**：中英文界面切换
3. **智能布局**：
   - 左侧导航栏（可拖拽区域）
   - 顶部系统三色灯（macOS）
   - 响应式主内容区
4. **AI 提供商管理**：
   - 预设提供商（通义千问/OpenAI/Anthropic/Ollama）
   - 自定义提供商配置
   - API Key 安全掩码显示

---

## 🚀 部署方案

### 开发模式
```bash
npm install
npm run tauri dev
```

### 生产构建
```bash
npm run tauri build
```

### Docker 部署
- 提供完整的 Docker 配置
- 支持多平台构建（Linux/macOS/Windows）

---

## 📈 项目亮点

### 1. 完全本地化运行
- 所有数据存储在本地
- 无云端依赖（除 AI API 调用）
- 隐私保护优先

### 2. 强大的工具生态
- 多引擎搜索（DuckDuckGo/Bing/Baidu）
- 天气/热搜快捷查询
- 浏览器自动化支持

### 3. 可扩展架构
- 插件化技能系统
- 可定制 Agent 系统提示
- 支持自定义 AI 提供商

### 4. 跨平台体验
- macOS：原生窗口装饰、系统托盘、隐私权限
- Linux：Wayland/X11 自动适配
- Windows：完整支持

---

## 🔮 未来改进方向

根据项目文档，后续可能增强：

1. **多模态能力**：
   - 本地图像识别（已通过 `tool_image_describe` 实现）
   - 语音转文字（media_understanding 模块）

2. **云同步**：
   - EvoMap 同步服务
   - 分布式记忆存储

3. **企业级功能**：
   - 飞书/钉钉企业版集成
   - 多租户支持

4. **性能优化**：
   - 向量数据库集成
   - 内存压缩算法

---

## 📝 总结

**Helix** 是一款设计精良、架构清晰的 AI 助理应用，展现了以下优势：

✅ **技术选型先进**：采用最新版 React 19/Tauri 2  
✅ **模块化设计**：高内聚低耦合的模块划分  
✅ **安全意识强**：沙箱机制 + 权限控制  
✅ **用户体验优秀**：响应式 UI + 暗黑模式  
✅ **扩展性良好**：插件系统 + 自定义提供商  

项目代码质量高，文档完善，是一个值得参考的现代桌面应用开发案例。

---

**报告生成时间**：2025年  
**项目版本**：v0.3.0  
**技术栈**：Tauri + React + Rust + SQLite

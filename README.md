# Helix

<div align="center">
  <img src="src-tauri/icons/icon-1024.png" alt="Helix Logo" width="120" height="120" style="border-radius: 24px;">

  <p>跨平台 AI 助理桌面应用</p>

  <p>
    <strong>简体中文</strong> |
    <a href="./README_EN.md">English</a>
  </p>
</div>

---

<div align="center">
  <img src="docs/images/app-preview.png" alt="Helix 应用截图" width="700">
  <br>
  <em>Helix — 跨平台 AI 助理桌面应用</em>
</div>

---

## 简介

Helix 是一个基于 [Tauri v2](https://v2.tauri.app/) 构建的跨平台 AI 助理桌面应用。它通过消息平台（微信文件传输助手等）与用户交互，内置具备工具调用能力的 AI Agent，可执行 Shell 命令、读写文件、搜索网页、操控浏览器等操作，并支持技能扩展和定时任务。

**核心能力：**

- 💬 **消息平台集成** — 微信文件传输助手、飞书（钉钉、企业微信即将支持）
- 🤖 **AI Agent** — 支持工具调用的完整 Agent 循环，可执行 Shell、文件操作、网页搜索、浏览器自动化等
- 🧠 **长期记忆** — 跨会话的信息存储与召回
- 🧩 **技能系统** — 可安装、创建和管理的技能插件，支持从 Git 仓库安装
- ⏰ **定时任务** — 可配置的 Cron 计划任务

## 飞书机器人配置

在[飞书开放平台](https://open.feishu.cn/app)创建自建应用，启用「机器人」能力，并在「事件与回调」中开启**长连接**模式，添加 `im.message.receive_v1` 事件。

所需权限范围（Scopes）：

```json
{
  "scopes": {
    "tenant": [
      "aily:file:read",
      "aily:file:write",
      "application:application.app_message_stats.overview:readonly",
      "application:application:self_manage",
      "application:bot.menu:write",
      "cardkit:card:write",
      "contact:contact.base:readonly",
      "contact:user.employee_id:readonly",
      "corehr:file:download",
      "docs:document.content:read",
      "event:ip_list",
      "im:chat",
      "im:chat.access_event.bot_p2p_chat:read",
      "im:chat.members:bot_access",
      "im:message",
      "im:message.group_at_msg:readonly",
      "im:message.group_msg",
      "im:message.p2p_msg:readonly",
      "im:message:readonly",
      "im:message:send_as_bot",
      "im:resource",
      "sheets:spreadsheet",
      "wiki:wiki:readonly"
    ],
    "user": [
      "aily:file:read",
      "aily:file:write",
      "contact:contact.base:readonly",
      "im:chat.access_event.bot_p2p_chat:read"
    ]
  }
}
```

## 技术栈

| 层级 | 技术 |
|------|------|
| 框架 | Tauri v2 |
| 前端 | React 19 + TypeScript + Ant Design |
| 后端 | Rust + Axum |
| 样式 | Tailwind CSS |
| 数据 | SQLite (rusqlite) |

## 快速开始

```bash
# 安装依赖
npm install

# 启动 Tauri 开发模式
npm run tauri dev

# 构建
npm run tauri build
```

## 项目结构

```
helix/
├── src/            # React 前端
│   ├── pages/      # 页面（微信、技能、定时任务、设置等）
│   ├── components/ # UI 组件
│   └── stores/     # 状态管理 (Zustand)
├── src-tauri/      # Rust 后端
│   └── src/modules/  # 核心模块（agent、skills、memory、cron 等）
├── docker/         # Docker 部署
└── docs/           # 文档
```

## 许可证

[CC-BY-NC-SA-4.0](./LICENSE)

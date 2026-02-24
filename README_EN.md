# Helix

<div align="center">
  <img src="src-tauri/icons/icon-1024.png" alt="Helix Logo" width="120" height="120" style="border-radius: 24px;">

  <p>Cross-Platform AI Assistant Desktop App</p>

  <p>
    <a href="./README.md">简体中文</a> |
    <strong>English</strong>
  </p>
</div>

---

## Overview

Helix is a cross-platform AI assistant desktop application built with [Tauri v2](https://v2.tauri.app/). It interacts with users through messaging platforms (WeChat File Transfer Assistant, etc.), features a built-in AI Agent with tool-calling capabilities — executing shell commands, reading/writing files, searching the web, controlling browsers, and more — with support for extensible skills and scheduled tasks.

**Key Features:**

- 💬 **Messaging Integration** — WeChat File Transfer Assistant (Feishu, DingTalk, WeCom coming soon)
- 🤖 **AI Agent** — Full agent loop with tool calling: shell, file ops, web search, browser automation, etc.
- 🧠 **Long-term Memory** — Cross-session information storage and recall
- 🧩 **Skills System** — Installable, creatable, and manageable skill plugins with Git repo support
- ⏰ **Scheduled Tasks** — Configurable cron jobs
- 🖥️ **Server Management** — Multi-server connection and management

## Tech Stack

| Layer | Technology |
|-------|------------|
| Framework | Tauri v2 |
| Frontend | React 19 + TypeScript + Ant Design |
| Backend | Rust + Axum |
| Styling | Tailwind CSS |
| Database | SQLite (rusqlite) |

## Getting Started

```bash
# Install dependencies
npm install

# Start Tauri dev mode
npm run tauri dev

# Build
npm run tauri build
```

## Project Structure

```
helix/
├── src/            # React frontend
│   ├── pages/      # Pages (WeChat, Skills, Cron Jobs, Settings, etc.)
│   ├── components/ # UI components
│   └── stores/     # State management (Zustand)
├── src-tauri/      # Rust backend
│   └── src/modules/  # Core modules (agent, skills, memory, cron, etc.)
├── docker/         # Docker deployment
└── docs/           # Documentation
```

## License

[CC-BY-NC-SA-4.0](./LICENSE)

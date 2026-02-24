# Implementation Plan: WeChat Ghosting & API Key Fix

## Description
The user is experiencing complete silence from the WeChat File Helper agent integration because the `API Key` in their configuration is empty (`""`). When `agent_process_message` encounters an empty API Key, it correctly aborts processing, but `filehelper.rs` purposefully swallows this specific error to avoid spamming the user when they use File Helper normally. This created a silent failure state (Ghosting).

## Action Plan
1. **Remove Error Swallowing**: Modify `filehelper_poll_messages` in `src-tauri/src/modules/filehelper.rs` to **stop** silently swallowing the "未设置 API Key" error. If the user expects remote control, they must be told *why* it is explicitly failing.
2. **Robust Immediate Feedback**: Update the logic so that the `emoji` receipt is only sent if `agent_process_message` does not immediately fail (or modify `agent.rs` to handle API Key validation before we commit to sending the emoji).
3. **Notify User**: Explicitly inform the user in the conversation that they must paste their Volcengine/OpenAI API Key in the application's Settings page for the agent to start "talking" back, since the configuration is completely empty.

## Rationale
By removing the silent failure, the WeChat channel will explicitly return `❌ 未设置 API Key，无法执行 AI 远程控制。` whenever they type, immediately clarifying why the AI isn't processing their commands. This aligns perfectly with the "remote control channel" philosophy.

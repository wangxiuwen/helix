---
description: How to release a new version of Helix
---
# Helix Release Workflow

When the user asks you to "release a new version", "bump version", "打个tag发个版本", or similar release requests, follow these exact steps:

1. Use the `run_command` tool to run the following command:
```bash
npm run release
```

2. The script `scripts/release.mjs` is interactive. You may use `send_command_input` to answer the prompts. 
- It will ask for the new version number (e.g. `0.8.0`).
- It will ask if you want to push to remote (y/n). 

3. Wait for the script to finish executing and creating the commit and tag.

// turbo-all
Note: Since `npm run release` handles the file modifications (package.json, tauri.conf.json, Cargo.toml), git add, git commit, and git tag for you, do not manually edit those files unless the script fails.

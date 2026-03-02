---
description: how to release a new version of Helix
---

# Helix Release Workflow

When the user asks you to "release a new version", "bump version", "打个tag发个版本", or similar release requests, follow these exact steps:

// turbo-all

1. **Check for uncommitted changes first**. Run `git status --short` to see if there are any uncommitted changes. If there are, commit them before proceeding:
```bash
git status --short
```

2. **If there are uncommitted changes**, stage and commit them with a descriptive message:
```bash
git add -A && git commit -m "feat: <describe the changes>"
```
Ask the user for the commit message if it's not obvious from context.

3. **Run the release script**:
```bash
npm run release
```

4. The script `scripts/release.mjs` is interactive. You may use `send_command_input` to answer the prompts.
- It will ask if you want to proceed (y/N).
- It will ask for the new version number (e.g. `0.8.0`).
- It will ask if you want to push to remote (y/n).

5. Wait for the script to finish executing and creating the commit and tag.

Note: Since `npm run release` handles the file modifications (package.json, tauri.conf.json, Cargo.toml), git add, git commit, and git tag for you, do not manually edit those files unless the script fails.

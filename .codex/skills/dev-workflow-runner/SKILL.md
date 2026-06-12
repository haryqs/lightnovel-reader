---
name: dev-workflow-runner
description: Run the lightnovel-reader development workflow. Use when starting work, finishing work, validating changes, preparing commits, checking git state, or enforcing the project’s repeatable development process across machines and AI agents.
---

# Dev Workflow Runner

Use this skill to run a clean development session.

## Start Of Session

1. Run `git status -sb`.
2. Read:
   - `AGENTS.md`
   - `docs/dev-memory/PROJECT_MEMORY.md`
   - `docs/dev-memory/NEXT_ACTIONS.md`
   - relevant files under `docs/current-project/`
3. If working on protocol/platform/core boundaries, also read:
   - `docs/resource-library-plan/7_终局架构_多端与插件运行时.md`
   - `docs/resource-library-plan/8_桥接协议_v0.1.md`
4. Identify existing dirty files before editing.

## During Development

- Use `rg` for code search.
- Read files before editing.
- Keep changes scoped.
- Do not revert user changes.
- Do not introduce dependencies unless the decision is recorded.
- For frontend code, do not directly import `@tauri-apps/*` outside `src/platform/`.
- For Rust business logic, prefer `crates/reading-core`; Tauri commands should stay glue.

## End Of Session

Run what is available:

```powershell
node scripts/check-arch.mjs
node scripts/check-dev-memory.mjs
npm.cmd run build
cargo test --workspace
git diff --check
```

If `npm.cmd run build` cannot run because `node_modules` is missing, say so.
If `cargo test --workspace` cannot run because crates.io is unavailable, say so.

Update:

- `docs/dev-memory/DEV_LOG.md`
- `docs/dev-memory/NEXT_ACTIONS.md`
- `docs/dev-memory/DECISIONS.md` when needed

## Git

- Use Chinese commit messages.
- Do not commit `node_modules/`, `target/`, `dist/`.
- In nested repo setups, operate in the inner `lightnovel-reader` repository unless the user explicitly asks to commit the outer repo.

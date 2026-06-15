---
name: project-memory-maintainer
description: Maintain persistent project memory for lightnovel-reader. Use when development changes should be recorded, docs need to be updated after code changes, decisions need ADR-style notes, NEXT_ACTIONS needs pruning, or a new AI needs to preserve context across sessions, machines, or agents.
---

# Project Memory Maintainer

Use this skill to keep project memory synchronized with actual development.

## Required Files

- `docs/dev-memory/PROJECT_MEMORY.md`
- `docs/dev-memory/DECISIONS.md`
- `docs/dev-memory/DEV_LOG.md`
- `docs/dev-memory/NEXT_ACTIONS.md`
- `docs/dev-memory/SESSION_TEMPLATE.md`
- `docs/dev-memory/工程约定与陷阱.md`
- `docs/README.md`
- `docs/resource-library-plan/8_桥接协议_v0.1.md`

## Workflow

1. Start by reading `docs/dev-memory/PROJECT_MEMORY.md` and `docs/dev-memory/NEXT_ACTIONS.md`.
2. After code changes, update `docs/dev-memory/DEV_LOG.md` with:
   - date
   - completed work
   - changed files
   - verification commands and results
   - unverified items
   - next step
3. If a product or architecture decision changed, append a short entry to `docs/dev-memory/DECISIONS.md`.
4. If a task is completed, remove or revise it in `docs/dev-memory/NEXT_ACTIONS.md`.
5. If bridge/protocol behavior changed, update `docs/resource-library-plan/8_桥接协议_v0.1.md`.
6. Run `node scripts/check-dev-memory.mjs`.

## Rules

- Record facts that affect future work, not chatty progress notes.
- Do not claim a build/test passed unless it actually ran.
- If verification is blocked by missing dependencies, GUI limitations, or network, write that explicitly.
- Prefer short, dated entries.
- Keep memory files aligned with code reality.

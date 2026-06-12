---
name: architecture-guard
description: Enforce lightnovel-reader architecture boundaries. Use when modifying bridge protocol, platform adapters, Tauri commands, reading-core, plugin runtime, source plugins, storage schemas, or any code crossing TypeScript/Rust/platform boundaries.
---

# Architecture Guard

Use this skill whenever a change touches engine/core/platform boundaries.

## Architecture

```text
reading-core(Rust)
+ reader-engine(TypeScript)
+ thin platform shells(WebView)
```

Tauri is the first desktop shell, not the business layer.

## Hard Rules

1. Frontend code outside `src/platform/` must not import `@tauri-apps/*`.
2. Business logic belongs in `crates/reading-core`.
3. Tauri commands in `src-tauri/src/lib.rs` should move data between platform and core.
4. Wire fields use camelCase.
5. Protocol changes require synchronized edits:
   - `src/platform/protocol.ts`
   - `src/platform/tauri.ts`
   - `src/platform/index.ts`
   - Rust command/serde structures
   - `docs/resource-library-plan/8_桥接协议_v0.1.md`
6. Do not reintroduce `type`/`kind` drift in annotation wire data.

## Checks

Run:

```powershell
node scripts/check-arch.mjs
git diff --check
```

When dependencies are present, also run:

```powershell
npm.cmd run build
cargo test --workspace
```

## Known Traps

- WebView2 image scheme differs on Windows; preserve `reader-img` handling.
- FTS5 CJK search uses trigram; do not replace with default unicode61.
- `bookId` must stay SHA-256 first 32 hex across frontend, core, and library.
- Tauri invoke sends camelCase parameters and maps to Rust snake_case.

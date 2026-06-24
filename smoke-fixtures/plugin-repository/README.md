# Plugin repository smoke fixture

This is a frozen source-plugin fixture used by `npm run smoke:plugin-repository`
(a real Tauri window smoke). It is loaded over real HTTPS from
`raw.githubusercontent.com` so the official plugin repository download path
exercises the same `rustls`/webpki TLS verification as production.

Contents:

- `aozora-smoke-source.zip` — a minimal legal source-plugin package
  (`public-domain`, not `user-declared`, not `official-free + acquire`) that
  passes official-repository qualification. It installs for policy/UI
  verification only; the runtime never executes its JS.
- `repository.json` — the official index (`schemaVersion 0.1`) pointing at the
  zip above with its real SHA-256.

The default smoke URL in `scripts/tauri-plugin-repository-smoke.mjs` points at
the `main` ref of `repository.json`. Regenerate this fixture with:

```powershell
npm.cmd run smoke:plugin-repository-fixtures -- `
  --out-dir <temp> `
  --base-url https://raw.githubusercontent.com/haryqs/lightnovel-reader/main/smoke-fixtures/plugin-repository `
  --plugin-id aozora-smoke-source
```

then copy `aozora-smoke-source.zip` + `repository.json` back here and commit
both together (the SHA-256 in `repository.json` must match the committed zip).

# Plugin repository smoke fixture

This is the canonical, in-repo fixture used by `npm run smoke:plugin-repository`
(a real Tauri window smoke). `scripts/tauri-plugin-repository-smoke.mjs` loads
the **official plugin index over real HTTPS**, exercising the same
`rustls`/webpki TLS verification as production.

Contents:

- `aozora-smoke-source.zip` — a minimal legal source-plugin package
  (`public-domain`, not `user-declared`, not `official-free + acquire`) that
  passes official-repository qualification. It installs for policy/UI
  verification only; the runtime never executes its JS.
- `repository.json` — the official index (`schemaVersion 0.1`) pointing at the
  zip above with its real SHA-256.

## How it is served (public gist mirror)

The main repo is **private** during development, so `raw.githubusercontent.com`
404s for the unauthenticated `reqwest::Client::new()` the plugin download
command uses. The fixture is therefore **mirrored to a public gist**, which
`gist.githubusercontent.com` serves over HTTPS with a GitHub certificate that
`rustls`/webpki trusts.

- Gist: https://gist.github.com/haryqs/a20cbbeecfb11a744b2650c776f0b615
- Index URL (smoke default):
  `https://gist.githubusercontent.com/haryqs/a20cbbeecfb11a744b2650c776f0b615/raw/repository.json`
- `packageUrl` inside the index points at the gist-served zip, so the in-repo
  `repository.json` and the gist mirror stay byte-identical.

`gh gist create` rejects binary files, so the mirror is pushed as a normal git
remote (a gist is a git repo). When the repo is later made public, the smoke can
switch back to `raw.githubusercontent.com/.../main/...` if desired — the
in-repo fixture is already the source of truth.

## Regenerate / re-mirror

```powershell
# 1. regenerate the canonical fixture into a temp dir
npm.cmd run smoke:plugin-repository-fixtures -- `
  --out-dir <temp> `
  --base-url https://gist.githubusercontent.com/haryqs/a20cbbeecfb11a744b2650c776f0b615 `
  --plugin-id aozora-smoke-source

# 2. copy aozora-smoke-source.zip + repository.json back here and commit
#    (the SHA-256 in repository.json must match the committed zip)

# 3. push the same two files to the gist so the public mirror matches:
git clone https://gist.github.com/a20cbbeecfb11a744b2650c776f0b615.git
#    ... overwrite aozora-smoke-source.zip + repository.json, commit, git push
```

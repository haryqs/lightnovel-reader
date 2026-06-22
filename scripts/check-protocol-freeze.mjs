import { readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = join(dirname(fileURLToPath(import.meta.url)), '..')

function read(relPath) {
  return readFileSync(join(root, relPath), 'utf8')
}

function fail(message) {
  console.error(`check-protocol-freeze: ${message}`)
  process.exitCode = 1
}

function unique(values) {
  return [...new Set(values)].sort()
}

const protocolTs = read('src/platform/protocol.ts')
const tauriLib = read('src-tauri/src/lib.rs')
const protocolDoc = read('docs/resource-library-plan/8_桥接协议_v0.1.md')

const versionMatch = protocolTs.match(/PROTOCOL_VERSION\s*=\s*['"]([^'"]+)['"]/)
if (!versionMatch) {
  fail('missing PROTOCOL_VERSION in src/platform/protocol.ts')
}
const version = versionMatch?.[1]

if (version && !protocolDoc.includes(`v${version}`)) {
  fail(`protocol document does not mention v${version}`)
}
if (version && !protocolDoc.includes(`PROTOCOL_VERSION = '${version}'`)) {
  fail(`protocol document does not pin PROTOCOL_VERSION = '${version}'`)
}

const errorUnionMatch = protocolTs.match(
  /export type BridgeErrorCode\s*=\s*([\s\S]*?)export interface BridgeError/,
)
if (!errorUnionMatch) {
  fail('missing BridgeErrorCode union in src/platform/protocol.ts')
}
const tsErrorCodes = unique(
  [...(errorUnionMatch?.[1] ?? '').matchAll(/['"]([a-zA-Z][a-zA-Z0-9]*)['"]/g)].map(
    (match) => match[1],
  ),
)
if (tsErrorCodes.length === 0) {
  fail('BridgeErrorCode union has no string literal codes')
}

const rustErrorCodes = unique(
  [...tauriLib.matchAll(/Self::(?:new|with_details)\("([^"]+)"/g)].map((match) => match[1]),
)
if (rustErrorCodes.length === 0) {
  fail('Rust BridgeError constructors do not expose any codes')
}

for (const code of rustErrorCodes) {
  if (!tsErrorCodes.includes(code)) {
    fail(`Rust BridgeError code "${code}" is missing from BridgeErrorCode`)
  }
}

for (const code of tsErrorCodes) {
  if (!protocolDoc.includes(`\`${code}\``)) {
    fail(`BridgeErrorCode "${code}" is missing from protocol document`)
  }
}

const freezePhrases = ['新增消息/新增可选字段', '不允许改名', '不允许', '删字段']
for (const phrase of freezePhrases) {
  if (!protocolDoc.includes(phrase)) {
    fail(`protocol freeze rule phrase missing: ${phrase}`)
  }
}

if (process.exitCode) {
  process.exit()
}

console.log(
  `check-protocol-freeze: OK(version=${version}, tsCodes=${tsErrorCodes.length}, rustCodes=${rustErrorCodes.length})`,
)

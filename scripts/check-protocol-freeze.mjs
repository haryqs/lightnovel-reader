import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

const protocolPath = resolve('src/platform/protocol.ts')
const protocolDocPath = resolve('docs/resource-library-plan/8_桥接协议_v0.1.md')
const tauriPath = resolve('src-tauri/src/lib.rs')

const protocol = readFileSync(protocolPath, 'utf8')
const protocolDoc = readFileSync(protocolDocPath, 'utf8')
const tauri = readFileSync(tauriPath, 'utf8')

const failures = []

const protocolVersion = protocol.match(/export const PROTOCOL_VERSION = '([^']+)'/)?.[1]
if (!protocolVersion) {
  failures.push('src/platform/protocol.ts missing PROTOCOL_VERSION')
} else {
  if (!protocolDoc.includes(`# 桥接协议 v${protocolVersion}`)) {
    failures.push(`protocol doc title does not mention v${protocolVersion}`)
  }
  if (!protocolDoc.includes(`PROTOCOL_VERSION = '${protocolVersion}'`)) {
    failures.push(`protocol doc version section does not mention PROTOCOL_VERSION = '${protocolVersion}'`)
  }
}

const codeBlock = protocol.match(/export type BridgeErrorCode =([\s\S]*?)\r?\n\r?\nexport interface BridgeError/)?.[1]
const protocolCodes = codeBlock ? [...codeBlock.matchAll(/\|\s*'([^']+)'/g)].map((m) => m[1]) : []
if (protocolCodes.length === 0) {
  failures.push('src/platform/protocol.ts missing BridgeErrorCode literals')
}

const docCodeSection = protocolDoc.match(/错误码清单[\s\S]*?\r?\n\| code \|[\s\S]*?\r?\n\r?\n\*\*当前已采用结构化错误码的 promise 消息\*\*/)?.[0]
const docCodes = docCodeSection ? [...docCodeSection.matchAll(/\| `([^`]+)` \|/g)].map((m) => m[1]) : []
if (docCodes.length === 0) {
  failures.push('protocol doc missing BridgeError error-code table')
}

const sameSet = (left, right) => {
  const a = [...left].sort()
  const b = [...right].sort()
  return a.length === b.length && a.every((value, index) => value === b[index])
}

if (protocolCodes.length > 0 && docCodes.length > 0 && !sameSet(protocolCodes, docCodes)) {
  failures.push(
    `BridgeErrorCode mismatch: protocol=[${protocolCodes.join(', ')}], doc=[${docCodes.join(', ')}]`,
  )
}

const tsShellOnlyCodes = new Set(['platformError'])
for (const code of protocolCodes) {
  if (tsShellOnlyCodes.has(code)) continue
  if (!tauri.includes(`"${code}"`)) {
    failures.push(`src-tauri/src/lib.rs does not construct BridgeError code "${code}"`)
  }
}

const rustCodes = [...tauri.matchAll(/Self::(?:new|with_details)\("([^"]+)"/g)].map((m) => m[1])
for (const code of new Set(rustCodes)) {
  if (!protocolCodes.includes(code)) {
    failures.push(`src-tauri/src/lib.rs constructs undocumented BridgeError code "${code}"`)
  }
}

if (failures.length > 0) {
  console.error('check-protocol-freeze: FAILED')
  for (const failure of failures) console.error(`  - ${failure}`)
  process.exit(1)
}

console.log(
  `check-protocol-freeze: OK(PROTOCOL_VERSION ${protocolVersion}, BridgeError codes ${protocolCodes.length})`,
)

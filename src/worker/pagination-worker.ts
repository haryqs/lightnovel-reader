/**
 * 分页 Web Worker —— 在独立线程中运行虚拟分页计算，不阻塞主线程 UI。
 *
 * 主路径：Rust/WASM paginate()（reading-core，74KB）
 * 兜底：TypeScript buildVirtualPages()（算法相同，但依赖 DOMParser）
 */
import init, { paginate } from './reading-core-wasm/reading_core.js'

// ---- 类型 ----

interface PaginateRequest {
  type: 'paginate'
  key: string
  html: string
  capacity: number
}

interface PaginateResponse {
  type: 'paginated'
  key: string
  pages: string[]
  /** 耗时 ms（用于性能监控） */
  elapsedMs: number
  /** 使用的引擎：'wasm' 或 'ts-fallback' */
  engine: 'wasm' | 'ts-fallback'
}

// ---- WASM 初始化 ----

let wasmReady = false
let wasmInitError: string | null = null

init()
  .then(() => {
    wasmReady = true
  })
  .catch((err: unknown) => {
    wasmInitError = err instanceof Error ? err.message : String(err)
    console.warn('[pagination-worker] WASM init failed, using TS fallback:', wasmInitError)
  })

// ---- TS fallback（与 reader-core.ts 的 buildVirtualPages 算法一致）----

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
}

function estimateBlockCost(html: string, textLength: number, capacity: number): number {
  const lower = html.toLowerCase()
  const imgCount = (lower.match(/<img\b/gi) || []).length
  const imgCost = Math.floor(imgCount * capacity * 0.62)
  const headingCost = /^<h[1-6]\b/i.test(html.trim()) ? 90 : 0
  const structuralCost = /<(table|svg|pre|blockquote)\b/i.test(lower) ? 160 : 28
  return Math.max(40, textLength + imgCost + headingCost + structuralCost)
}

function splitText(text: string, target: number): string[] {
  const chunks: string[] = []
  let rest = text.trim()
  while (rest.length > target) {
    const total = rest.length
    const start = Math.floor(target * 0.72)
    const end = Math.min(total, Math.floor(target * 1.12))
    const window = rest.slice(start, end)
    const punctPositions = ['。', '！', '？', ';', '；']
      .map(p => window.lastIndexOf(p))
      .filter(i => i >= 0)
    let cut = punctPositions.length > 0 ? start + Math.max(...punctPositions) + 1 : target
    if (cut < Math.floor(target * 0.55)) cut = target
    chunks.push(rest.slice(0, cut).trim())
    rest = rest.slice(cut).trim()
  }
  if (rest) chunks.push(rest)
  return chunks
}

function wrapTextLikeElement(el: Element, text: string, keepId: boolean): string {
  const tag = el.tagName.toLowerCase()
  const attrs: string[] = []
  const cls = el.getAttribute('class')
  const style = el.getAttribute('style')
  const id = el.getAttribute('id')
  if (keepId && id) attrs.push(`id="${escapeHtml(id)}"`)
  if (cls) attrs.push(`class="${escapeHtml(cls)}"`)
  if (style) attrs.push(`style="${escapeHtml(style)}"`)
  return `<${tag}${attrs.length ? ' ' + attrs.join(' ') : ''}>${escapeHtml(text)}</${tag}>`
}

interface PageBlock {
  html: string
  textLength: number
}

function elementToPageBlocks(el: Element, capacity: number): PageBlock[] {
  const html = el.outerHTML
  const text = (el.textContent || '').replace(/\s+/g, ' ').trim()
  const hasComplex = Boolean(el.querySelector('img,svg,table,math,video,audio,canvas'))
  const tag = el.tagName.toLowerCase()
  const canSplit =
    ['p', 'div', 'li', 'blockquote'].includes(tag) &&
    !hasComplex &&
    el.children.length <= 2 &&
    text.length > capacity * 1.15

  if (!canSplit) return [{ html, textLength: text.length }]

  const target = Math.max(220, Math.floor(capacity * 0.78))
  const chunks = splitText(text, target)
  return chunks.map((chunk, i) => ({
    html: wrapTextLikeElement(el, chunk, i === 0),
    textLength: chunk.length,
  }))
}

function buildVirtualPages(html: string, capacity: number): string[] {
  const doc = new DOMParser().parseFromString(html, 'text/html')
  const blocks = Array.from(doc.body.children).flatMap(el =>
    elementToPageBlocks(el as Element, capacity),
  )

  if (blocks.length === 0) {
    const text = doc.body.textContent?.trim()
    return text ? [`<p>${escapeHtml(text)}</p>`] : ['']
  }

  const pages: string[] = []
  let current: string[] = []
  let used = 0
  for (const block of blocks) {
    const cost = estimateBlockCost(block.html, block.textLength, capacity)
    if (current.length > 0 && used + cost > capacity) {
      pages.push(current.join(''))
      current = []
      used = 0
    }
    current.push(block.html)
    used += cost
  }
  if (current.length > 0) pages.push(current.join(''))
  return pages.length > 0 ? pages : ['']
}

// ---- Worker 消息处理 ----

self.onmessage = (e: MessageEvent<PaginateRequest>) => {
  if (e.data.type !== 'paginate') return
  const { key, html, capacity } = e.data
  const start = performance.now()

  let pages: string[]
  let engine: 'wasm' | 'ts-fallback'

  if (wasmReady) {
    try {
      pages = paginate(html, capacity)
      engine = 'wasm'
    } catch (err: unknown) {
      console.warn('[pagination-worker] WASM call failed, TS fallback:', err)
      pages = buildVirtualPages(html, capacity)
      engine = 'ts-fallback'
    }
  } else {
    pages = buildVirtualPages(html, capacity)
    engine = 'ts-fallback'
  }

  const elapsedMs = Math.round(performance.now() - start)
  const response: PaginateResponse = { type: 'paginated', key, pages, elapsedMs, engine }
  self.postMessage(response)
}

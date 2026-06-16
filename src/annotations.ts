// 标注核心引擎：文本锚点计算、高亮渲染、持久化 CRUD
import { bridge, type Annotation, type TextAnchor } from './platform'

// 数据结构定义在桥接协议里（与 Rust storage::Annotation 一一对应），这里转发
export type { Annotation, Locator, TextAnchor } from './platform'

export type HighlightColor = 'yellow' | 'red' | 'blue' | 'green'

export const HIGHLIGHT_COLORS: Record<HighlightColor, { bg: string; border: string }> = {
  yellow: { bg: 'rgba(255,235,59,0.35)', border: '#f9a825' },
  red:    { bg: 'rgba(244,67,54,0.25)', border: '#c62828' },
  blue:   { bg: 'rgba(33,150,243,0.25)', border: '#1565c0' },
  green:  { bg: 'rgba(76,175,80,0.25)', border: '#2e7d32' },
}

// ---- 计算选中文本在章节中的锚点 ----
// 偏移一律用 cloneContents().textContent 计算，与定位用的 contentEl.textContent 同口径。
// 不用 range.toString()/preRange.toString()——它们在块级边界插入 \n，跨段落选择时
// 会让 exact 带上 textContent 里没有的换行，导致后续 indexOf 定位失败、跨元素高亮丢失。
export function computeAnchor(): TextAnchor | null {
  const sel = window.getSelection()
  if (!sel || sel.isCollapsed || !sel.rangeCount) return null

  const range = sel.getRangeAt(0)
  const contentEl = document.querySelector('.reader-content')
  if (!contentEl) return null

  const fullText = contentEl.textContent || ''

  const preRange = document.createRange()
  preRange.setStart(contentEl, 0)
  preRange.setEnd(range.startContainer, range.startOffset)
  const start = (preRange.cloneContents().textContent || '').length
  const end = start + (range.cloneContents().textContent || '').length
  const exact = fullText.slice(start, end)
  if (exact.trim().length === 0) return null

  // 前 20 / 后 20 上下文（用于定位消歧）
  const prefix = fullText.substring(Math.max(0, start - 20), start)
  const suffix = fullText.substring(end, Math.min(fullText.length, end + 20))

  return { start, end, exact, prefix, suffix }
}

// ---- 在 DOM 中应用高亮 ----
export function applyHighlight(range: Range, color: HighlightColor, id: string): HTMLElement | null {
  try {
    const palette = HIGHLIGHT_COLORS[color]
    const mark = document.createElement('mark')
    mark.className = `hl hl-${color}`
    mark.dataset.annotationId = id
    mark.style.cssText = `
      background: ${palette.bg}; border-bottom: 1.5px solid ${palette.border};
      cursor: pointer; border-radius: 2px; padding: 1px 0;
    `
    range.surroundContents(mark)
    return mark
  } catch {
    // 跨元素选择，surroundContents 会报错。降级到用 CSS highlight API 或跳过。
    return null
  }
}

// ---- 在清洗后的全文里稳健定位标注 ----
// 字号/重排不改变 textContent，但 exact 文本可能在章内重复出现（人名、常用短语）。
// 取所有出现位置，用 prefix/suffix 上下文消歧，再以保存时的 start 就近兜底，
// 避免"永远高亮第一个匹配"的错位。返回起始偏移，找不到返回 -1。
export function locateAnnotationOffset(fullText: string, anchor: TextAnchor): number {
  const { exact, prefix, suffix, start } = anchor
  if (!exact) return -1

  const occ: number[] = []
  for (let i = fullText.indexOf(exact); i !== -1; i = fullText.indexOf(exact, i + 1)) {
    occ.push(i)
    if (occ.length > 64) break // 防极端重复文本下的退化
  }
  if (occ.length === 0) {
    // exact 已变（清洗规则变更等）：退一步用 prefix+exact 上下文找
    if (prefix) {
      const ctx = fullText.indexOf(prefix + exact)
      if (ctx >= 0) return ctx + prefix.length
    }
    return -1
  }
  if (occ.length === 1) return occ[0]

  // 多个匹配：prefix/suffix 命中各 +2 分，再用与 start 的距离做次级打分（越近越好）
  let best = occ[0]
  let bestScore = -Infinity
  for (const off of occ) {
    let score = 0
    if (prefix && fullText.slice(Math.max(0, off - prefix.length), off).endsWith(prefix)) score += 2
    const after = off + exact.length
    if (suffix && fullText.slice(after, after + suffix.length).startsWith(suffix)) score += 2
    score -= Math.abs(off - start) / Math.max(1, fullText.length) // 0..1 惩罚
    if (score > bestScore) { bestScore = score; best = off }
  }
  return best
}

// ---- 渲染已保存的标注到页面 ----
export function renderAnnotations(annotations: Annotation[], currentHref: string) {
  const contentEl = document.querySelector('.reader-content')
  if (!contentEl) return
  for (const ann of annotations) {
    if (ann.locator.chapterHref === currentHref) applyAnnotationHighlight(ann, contentEl)
  }
}

// ---- 渲染单条标注（跨元素：按偏移逐文本节点包裹，不依赖 surroundContents 整段成功）----
export function applyAnnotationHighlight(ann: Annotation, root?: Element): boolean {
  const contentEl = root || document.querySelector('.reader-content')
  if (!contentEl) return false
  // 已渲染则跳过，避免重复包裹成嵌套 mark
  if (contentEl.querySelector(`mark[data-annotation-id="${ann.id}"]`)) return true

  const { exact } = ann.locator.anchor
  if (!exact) return false
  const fullText = contentEl.textContent || ''
  const offset = locateAnnotationOffset(fullText, ann.locator.anchor)
  if (offset === -1) return false

  wrapTextAtOffset(contentEl, offset, offset + exact.length, ann.id, (ann.color || 'yellow') as HighlightColor)
  return true
}

// ---- 在指定偏移处包裹高亮 ----
function wrapTextAtOffset(
  root: Node, targetStart: number, targetEnd: number,
  annId: string, color: HighlightColor
) {
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT)
  let currentOffset = 0
  const matches: { node: Text; localStart: number; localEnd: number }[] = []

  let node: Text | null
  while ((node = walker.nextNode() as Text | null)) {
    const len = node.textContent?.length || 0
    const nodeStart = currentOffset
    const nodeEnd = currentOffset + len

    if (nodeEnd > targetStart && nodeStart < targetEnd) {
      const localStart = Math.max(0, targetStart - nodeStart)
      const localEnd = Math.min(len, targetEnd - nodeStart)
      matches.push({ node, localStart, localEnd })
    }

    currentOffset += len
    if (currentOffset > targetEnd) break
  }

  for (const { node, localStart, localEnd } of matches) {
    const range = document.createRange()
    range.setStart(node, localStart)
    range.setEnd(node, localEnd)
    try {
      applyHighlight(range, color, annId)
    } catch { /* 跳过失败的包裹 */ }
  }
}

// ---- 后端 CRUD ----
export async function saveAnnotation(ann: Annotation): Promise<void> {
  await bridge.saveAnnotation(ann)
}

export async function loadAnnotations(bookId: string): Promise<Annotation[]> {
  return await bridge.listAnnotations(bookId)
}

export async function deleteAnnotation(id: string): Promise<void> {
  await bridge.deleteAnnotation(id)
}

// ---- Book ID: SHA-256 内容哈希（改名/移动后标注仍对上）----
export async function computeBookId(data: ArrayBuffer): Promise<string> {
  const digest = await crypto.subtle.digest('SHA-256', data)
  return Array.from(new Uint8Array(digest))
    .map(b => b.toString(16).padStart(2, '0'))
    .join('')
    .slice(0, 32)
}

// ---- 导出 Markdown (Obsidian 兼容) ----
export interface ExportContext {
  bookTitle: string
  chapterTitles: Map<string, string>  // spineHref → 章标题
}

export function exportAnnotations(anns: Annotation[], ctx: ExportContext): string {
  if (anns.length === 0) return ''

  const byChapter = new Map<string, Annotation[]>()
  for (const a of anns) {
    const href = a.locator.chapterHref
    if (!byChapter.has(href)) byChapter.set(href, [])
    byChapter.get(href)!.push(a)
  }

  // 按章节内偏移排序
  for (const list of byChapter.values()) {
    list.sort((a, b) => a.locator.anchor.start - b.locator.anchor.start)
  }

  let md = `# ${ctx.bookTitle} 标注导出\n\n`
  md += `> 导出时间：${new Date().toLocaleString('zh-CN')}\n`
  md += `> 共 ${anns.length} 条标注\n\n---\n\n`

  for (const [href, items] of byChapter) {
    const chapterTitle = ctx.chapterTitles.get(href) || href.split('/').pop() || href
    md += `## ${chapterTitle}\n\n`

    for (const a of items) {
      const { exact, start } = a.locator.anchor
      const estPage = Math.floor(start / 1000) + 1  // 粗略估页

      md += `> ${exact}\n`
      md += `> — 第 ${estPage} 页\n\n`

      if (a.note && a.note.trim()) {
        md += `${a.note.trim()}\n\n`
      }

      md += `---\n\n`
    }
  }

  return md
}

/// 完整结构化导出：保留每条标注的全部字段，可被外部工具或回导完整还原。
export function exportAnnotationsJson(anns: Annotation[], ctx: ExportContext): string {
  const sorted = [...anns].sort((a, b) => {
    const h = a.locator.chapterHref.localeCompare(b.locator.chapterHref)
    return h !== 0 ? h : a.locator.anchor.start - b.locator.anchor.start
  })
  const doc = {
    schema: 'lightnovel-reader/annotations',
    version: 1,
    bookTitle: ctx.bookTitle,
    exportedAt: new Date().toISOString(),
    count: anns.length,
    annotations: sorted.map(a => ({
      id: a.id,
      kind: a.kind,
      color: a.color ?? null,
      chapterHref: a.locator.chapterHref,
      chapterTitle: ctx.chapterTitles.get(a.locator.chapterHref) ?? null,
      anchor: a.locator.anchor,            // exact / start / end / prefix / suffix 全保留
      note: a.note ?? null,
      createdAt: a.createdAt,
      updatedAt: a.updatedAt,
    })),
  }
  return JSON.stringify(doc, null, 2)
}

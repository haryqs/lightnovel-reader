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
export function computeAnchor(): TextAnchor | null {
  const sel = window.getSelection()
  if (!sel || sel.isCollapsed || !sel.rangeCount) return null

  const range = sel.getRangeAt(0)
  const contentEl = document.querySelector('.reader-content')
  if (!contentEl) return null

  // 获取 contentEl 的完整文本
  const fullText = contentEl.textContent || ''
  const selectedText = range.toString().trim()
  if (selectedText.length === 0) return null

  // 计算选中文本在全文中的起始偏移
  const preRange = document.createRange()
  preRange.setStart(contentEl, 0)
  preRange.setEnd(range.startContainer, range.startOffset)
  const start = preRange.toString().length
  const end = start + selectedText.length

  // 前 20 / 后 20 上下文
  const prefix = fullText.substring(Math.max(0, start - 20), start)
  const suffix = fullText.substring(end, Math.min(fullText.length, end + 20))

  return { start, end, exact: selectedText, prefix, suffix }
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

// ---- 渲染已保存的标注到页面 ----
export function renderAnnotations(annotations: Annotation[], currentHref: string) {
  const contentEl = document.querySelector('.reader-content')
  if (!contentEl) return

  const chapterAnns = annotations.filter(
    a => a.locator.chapterHref === currentHref
  )

  const fullText = contentEl.textContent || ''

  for (const ann of chapterAnns) {
    const { exact, prefix } = ann.locator.anchor
    if (!exact) continue

    // 在全文里定位（先用 exact 精确匹配，再用 prefix+exact 兜底）
    let offset = fullText.indexOf(exact)
    if (offset === -1 && prefix) {
      const ctx = prefix + exact
      offset = fullText.indexOf(ctx)
      if (offset >= 0) offset += prefix.length
    }
    if (offset === -1) continue

    // 在 DOM 中找到对应文本节点并包裹 <mark>
    wrapTextAtOffset(contentEl, offset, offset + exact.length, ann.id, (ann.color || 'yellow') as HighlightColor)
  }
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

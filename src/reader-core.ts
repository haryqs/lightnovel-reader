import { bridge, type BookInfo, type TocItem } from './platform'
import { readerThemes, baseTypography, type ThemeName } from './themes'
import { type Annotation, type HighlightColor, computeAnchor, applyHighlight, renderAnnotations, saveAnnotation, loadAnnotations, deleteAnnotation, computeBookId, exportAnnotations, exportAnnotationsJson, type ExportContext } from './annotations'

// 书籍结构类型定义在桥接协议里,这里转发给既有调用方
export type { BookInfo, TocItem, SpineItem } from './platform'

export interface LocationInfo {
  percentage: number
  href: string
}

export type PageMode = 'single' | 'double'

export interface ReaderLayoutSettings {
  doubleMargin: number
  doublePadding: number
  doubleGutter: number
}

export class ReaderCore {
  bookInfo: BookInfo | null = null
  private currentChapter = ''
  private fontSize = 100
  private theme: ThemeName = 'sepia'
  private pageMode: PageMode = 'single'
  private layoutSettings: ReaderLayoutSettings = {
    doubleMargin: 84,
    doublePadding: 42,
    doubleGutter: 108,
  }
  private viewer: HTMLElement | null = null
  private resizeObserver: ResizeObserver | null = null
  private locationFrame: number | null = null
  private chapterLoadSeq = 0
  private repaginateFrame: number | null = null
  private currentChapterBase = ''
  private currentChapterHtml = ''
  private currentPages: string[] = []
  private currentPageIndex = 0
  private pageModelCache = new Map<string, string[]>()
  private readonly maxPageModels = 8

  // 标注
  annotations: Annotation[] = []
  private bookId = ''
  onAnnotationsChanged: ((anns: Annotation[]) => void) | null = null

  onRelocated: ((info: LocationInfo) => void) | null = null
  onReady: (() => void) | null = null

  async openFromBuffer(data: ArrayBuffer, viewerEl: HTMLElement) {
    this.resetRuntimeCaches()
    this.viewer = viewerEl
    this.bookId = await computeBookId(data)
    this.bookInfo = await bridge.openBookFromBytes(new Uint8Array(data))
    this.onReady?.()

    // 加载已有标注
    this.annotations = await loadAnnotations(this.bookId).catch(() => [])

    await this.openAtSavedPosition()
  }

  // 按路径开书（Calibre 书架点击进入）。bookId 由后端按内容哈希返回，与文件选择器开书一致。
  async openFromPath(path: string, viewerEl: HTMLElement) {
    this.resetRuntimeCaches()
    this.viewer = viewerEl
    const opened = await bridge.openBookFromPath(path)
    this.bookId = opened.bookId
    this.bookInfo = opened.info
    this.onReady?.()
    this.annotations = await loadAnnotations(this.bookId).catch(() => [])
    await this.openAtSavedPosition()
  }

  // 自有书库按 id 开书，路径解析留在 core/平台壳一侧，前端不持有对象仓库规则。
  async openFromLibraryId(id: string, viewerEl: HTMLElement) {
    this.resetRuntimeCaches()
    this.viewer = viewerEl
    const opened = await bridge.openLibraryBook(id)
    this.bookId = opened.bookId
    this.bookInfo = opened.info
    this.onReady?.()
    this.annotations = await loadAnnotations(this.bookId).catch(() => [])
    await this.openAtSavedPosition()
  }

  // 开书落点:有保存的进度且章节仍在 spine 里 → 恢复;否则从头开始。
  private async openAtSavedPosition() {
    if (!this.bookInfo || this.bookInfo.spine.length === 0) return
    const saved = await bridge.getProgress(this.bookId).catch(() => null)
    const usable = saved && this.spineIndexOf(saved.chapterHref) >= 0 ? saved : null
    await this.loadChapter(usable ? usable.chapterHref : this.bookInfo.spine[0].href)
    if (usable && usable.chapterProgress > 0) {
      this.setChapterProgress(usable.chapterProgress)
    }
  }

  // ====== 章节加载 ======
  // 双缓冲：首次创建持久 wrapper，后续只换 innerHTML（避免全 DOM 重建和白屏）
  async loadChapter(href: string) {
    if (!this.viewer) return
    if (!href || !href.trim()) return

    const loadSeq = ++this.chapterLoadSeq
    const baseHref = href.split('#')[0]
    const html = await this.getChapterHtml(baseHref)
    if (loadSeq !== this.chapterLoadSeq) return

    this.currentChapter = this.canonicalChapterHref(href)

    // 首次加载：初始化持久 wrapper；后续只换内容
    let vp = this.getViewport()
    if (!vp) {
      this.viewer.innerHTML = `
        <style id="reader-styles"></style>
        <div class="reader-vp">
          <div class="reader-page-window">
            <div class="reader-content"></div>
          </div>
        </div>
      `
      vp = this.getViewport()
      if (!vp) return
      vp.addEventListener('scroll', () => this.scheduleEmitLocation(), { passive: true })
      this.resizeObserver?.disconnect()
      this.resizeObserver = new ResizeObserver(() => this.scheduleRepaginate())
      this.resizeObserver.observe(vp)
      const content = this.viewer.querySelector('.reader-content')!
      content.addEventListener('click', (e) => {
        const a = (e.target as HTMLElement).closest('a') as HTMLAnchorElement | null
        if (!a) return
        e.preventDefault()
        const href = a.getAttribute('href') || ''
        if (!href || /^(https?:|mailto:|tel:)/i.test(href)) return
        void this.navigateInternal(href)
      })
    }

    // 只更新样式和内容（不重建 DOM 树）
    this.getFrame()?.classList.toggle('reader-vp-double', this.pageMode === 'double')
    const styleEl = this.viewer.querySelector('#reader-styles')!
    styleEl.textContent = this.buildThemeStyles()
    this.currentChapterBase = baseHref
    this.currentChapterHtml = html
    this.currentPages = this.getOrBuildPageModel(baseHref, html)
    this.currentPageIndex = this.findInitialPageIndex(href)
    this.renderCurrentPages()

    // 后台预加载相邻章节，连续翻页跨章时减少等待。
    this.preloadAroundChapter(href)

    this.scheduleEmitLocation()
  }

  private chapterCache = new Map<string, string>()
  private chapterInflight = new Map<string, Promise<string>>()
  private readonly maxCachedChapters = 10

  private touchChapterCache(href: string, html: string) {
    this.chapterCache.delete(href)
    this.chapterCache.set(href, html)
    while (this.chapterCache.size > this.maxCachedChapters) {
      const first = this.chapterCache.keys().next().value
      if (!first) break
      this.chapterCache.delete(first)
    }
  }

  private async getChapterHtml(href: string): Promise<string> {
    const cached = this.chapterCache.get(href)
    if (cached !== undefined) {
      this.touchChapterCache(href, cached)
      return cached
    }

    const pending = this.chapterInflight.get(href)
    if (pending) return pending

    const request = bridge.getChapter(href)
      .then(html => {
        this.touchChapterCache(href, html)
        return html
      })
      .finally(() => {
        this.chapterInflight.delete(href)
      })
    this.chapterInflight.set(href, request)
    return request
  }

  // 后台预加载相邻章节 HTML（不阻塞当前阅读）
  private preloadAroundChapter(currentHref: string) {
    if (!this.bookInfo) return
    const idx = this.spineIndexOf(currentHref)
    if (idx < 0) return
    const candidates = [idx - 1, idx + 1, idx + 2]
    for (const i of candidates) {
      const item = this.bookInfo.spine[i]
      if (!item) continue
      const href = item.href.split('#')[0]
      if (this.chapterCache.has(href) || this.chapterInflight.has(href)) continue
      void this.getChapterHtml(href).catch(() => {})
    }
  }

  // 把书内相对链接（可带 #fragment）解析到 spine 章节并跳转。
  private getOrBuildPageModel(href: string, html: string): string[] {
    const key = this.pageModelKey(href)
    const cached = this.pageModelCache.get(key)
    if (cached) {
      this.pageModelCache.delete(key)
      this.pageModelCache.set(key, cached)
      return cached
    }

    const pages = this.buildVirtualPages(html)
    this.pageModelCache.set(key, pages)
    while (this.pageModelCache.size > this.maxPageModels) {
      const first = this.pageModelCache.keys().next().value
      if (!first) break
      this.pageModelCache.delete(first)
    }
    return pages
  }

  private pageModelKey(href: string): string {
    const vp = this.getViewport()
    const w = vp ? Math.round(vp.clientWidth / 16) * 16 : 0
    const h = vp ? Math.round(vp.clientHeight / 16) * 16 : 0
    const l = this.layoutSettings
    return [href, this.pageMode, this.fontSize, w, h, l.doubleMargin, l.doublePadding, l.doubleGutter].join('|')
  }

  private getEstimatedPageCapacity(): number {
    const vp = this.getViewport()
    const viewportWidth = Math.max(360, vp?.clientWidth || 960)
    const viewportHeight = Math.max(420, vp?.clientHeight || 720)
    const fontPx = 16 * (this.fontSize / 100)
    const linePx = fontPx * 1.7
    let pageWidth = Math.min(900, viewportWidth) - 48
    let pageHeight = viewportHeight - 32

    if (this.pageMode === 'double') {
      const l = this.layoutSettings
      const spreadWidth = Math.max(320, viewportWidth - l.doubleMargin * 2)
      pageWidth = (spreadWidth - l.doubleGutter) / 2 - l.doublePadding * 2
      pageHeight = viewportHeight - l.doubleMargin * 2 - l.doublePadding * 2
    }

    const charsPerLine = Math.max(12, Math.floor(pageWidth / (fontPx * 1.04)))
    const lines = Math.max(8, Math.floor(pageHeight / linePx))
    return Math.max(180, Math.floor(charsPerLine * lines * 0.82))
  }

  private buildVirtualPages(html: string): string[] {
    const capacity = this.getEstimatedPageCapacity()
    const doc = new DOMParser().parseFromString(html, 'text/html')
    const blocks = Array.from(doc.body.children)
      .flatMap(el => this.elementToPageBlocks(el as HTMLElement, capacity))

    if (blocks.length === 0) {
      const text = doc.body.textContent?.trim()
      return text ? [`<p>${this.escapeHtml(text)}</p>`] : ['']
    }

    const pages: string[] = []
    let current: string[] = []
    let used = 0
    for (const block of blocks) {
      const cost = this.estimateBlockCost(block.html, block.textLength, capacity)
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

  private elementToPageBlocks(el: HTMLElement, capacity: number): Array<{ html: string; textLength: number }> {
    const html = el.outerHTML
    const text = (el.textContent || '').replace(/\s+/g, ' ').trim()
    const hasComplexContent = Boolean(el.querySelector('img,svg,table,math,video,audio,canvas'))
    const tag = el.tagName.toLowerCase()
    const canSplit = ['p', 'div', 'li', 'blockquote'].includes(tag)
      && !hasComplexContent
      && el.children.length <= 2
      && text.length > capacity * 1.15

    if (!canSplit) return [{ html, textLength: text.length }]

    const chunks = this.splitText(text, Math.max(220, Math.floor(capacity * 0.78)))
    return chunks.map((chunk, idx) => ({
      html: this.wrapTextLikeElement(el, chunk, idx === 0),
      textLength: chunk.length,
    }))
  }

  private splitText(text: string, target: number): string[] {
    const chunks: string[] = []
    let rest = text.trim()
    while (rest.length > target) {
      let cut = -1
      const start = Math.floor(target * 0.72)
      const end = Math.min(rest.length, Math.floor(target * 1.12))
      const windowText = rest.slice(start, end)
      const punctuation = Math.max(
        windowText.lastIndexOf('。'),
        windowText.lastIndexOf('！'),
        windowText.lastIndexOf('？'),
        windowText.lastIndexOf(';'),
        windowText.lastIndexOf('；'),
      )
      if (punctuation >= 0) cut = start + punctuation + 1
      if (cut < Math.floor(target * 0.55)) cut = target
      chunks.push(rest.slice(0, cut).trim())
      rest = rest.slice(cut).trim()
    }
    if (rest) chunks.push(rest)
    return chunks
  }

  private wrapTextLikeElement(el: HTMLElement, text: string, keepId: boolean): string {
    const tag = el.tagName.toLowerCase()
    const attrs: string[] = []
    const className = el.getAttribute('class')
    const style = el.getAttribute('style')
    const id = el.getAttribute('id')
    if (keepId && id) attrs.push(`id="${this.escapeHtml(id)}"`)
    if (className) attrs.push(`class="${this.escapeHtml(className)}"`)
    if (style) attrs.push(`style="${this.escapeHtml(style)}"`)
    return `<${tag}${attrs.length ? ' ' + attrs.join(' ') : ''}>${this.escapeHtml(text)}</${tag}>`
  }

  private estimateBlockCost(html: string, textLength: number, capacity: number): number {
    const imageCost = (html.match(/<img\b/gi)?.length || 0) * Math.floor(capacity * 0.62)
    const headingCost = /^<h[1-6]\b/i.test(html.trim()) ? 90 : 0
    const structuralCost = /<(table|svg|pre|blockquote)\b/i.test(html) ? 160 : 28
    return Math.max(40, textLength + imageCost + headingCost + structuralCost)
  }

  private escapeHtml(value: string): string {
    return value
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;')
  }

  private findInitialPageIndex(href: string): number {
    const fragment = href.split('#')[1]
    if (!fragment) return 0
    const decoded = decodeURIComponent(fragment)
    const needles = [`id="${decoded}"`, `id='${decoded}'`, `name="${decoded}"`, `name='${decoded}'`]
    const idx = this.currentPages.findIndex(page => needles.some(n => page.includes(n)))
    return idx >= 0 ? this.normalizePageIndex(idx) : 0
  }

  private visiblePageCount(): number {
    return this.pageMode === 'double' ? 2 : 1
  }

  private normalizePageIndex(index: number): number {
    const max = Math.max(0, this.currentPages.length - 1)
    let next = Math.max(0, Math.min(max, Math.round(index)))
    if (this.pageMode === 'double') next = Math.floor(next / 2) * 2
    return Math.max(0, Math.min(max, next))
  }

  private lastPageIndexForMode(): number {
    const max = Math.max(0, this.currentPages.length - 1)
    return this.pageMode === 'double' ? Math.floor(max / 2) * 2 : max
  }

  private renderCurrentPages() {
    const content = this.viewer?.querySelector('.reader-content') as HTMLElement | null
    if (!content) return
    this.currentPageIndex = this.normalizePageIndex(this.currentPageIndex)
    const pages: string[] = []
    for (let i = 0; i < this.visiblePageCount(); i++) {
      const pageIndex = this.currentPageIndex + i
      const html = this.currentPages[pageIndex] || ''
      pages.push(`<section class="reader-page" data-page="${pageIndex + 1}">${html}</section>`)
    }
    content.innerHTML = pages.join('')
    if (this.annotations.length > 0) {
      requestAnimationFrame(() => renderAnnotations(this.annotations, this.currentChapter))
    }
    this.scheduleEmitLocation()
  }

  private scheduleRepaginate() {
    if (!this.currentChapterHtml || this.repaginateFrame !== null) return
    this.repaginateFrame = requestAnimationFrame(() => {
      this.repaginateFrame = null
      this.repaginateCurrentChapter()
    })
  }

  private repaginateCurrentChapter() {
    if (!this.currentChapterHtml || !this.currentChapterBase) return
    const progress = this.getChapterProgress()
    this.currentPages = this.getOrBuildPageModel(this.currentChapterBase, this.currentChapterHtml)
    this.setChapterProgress(progress)
  }

  private async navigateInternal(href: string) {
    if (!this.bookInfo) return
    const [base, frag] = href.split('#')
    const baseName = base.split('/').pop() || base
    const target = this.bookInfo.spine.find(s => {
      const sName = s.href.split('/').pop() || s.href
      return s.href === base || sName === baseName
    })
    if (target) {
      await this.loadChapter(frag ? `${target.href}#${frag}` : target.href)
    }
  }

  // 把任意来源的章节 href（可能带 opf 目录前缀或 #fragment）规范化为 spine 里的 href。
  private canonicalChapterHref(href: string): string {
    if (!this.bookInfo) return href
    const [base, frag] = href.split('#')
    const baseName = base.split('/').pop() || base
    const match = this.bookInfo.spine.find(
      s => s.href === base || (s.href.split('/').pop() || s.href) === baseName
    )
    if (!match) return href
    return frag ? `${match.href}#${frag}` : match.href
  }

  private buildThemeStyles(): string {
    // 样式仅在主题或字号变更时重建
    if (this.cachedStyles && this.cachedTheme === this.theme && this.cachedFontSize === this.fontSize) {
      return this.cachedStyles
    }
    this.cachedTheme = this.theme
    this.cachedFontSize = this.fontSize
    this.cachedStyles = this.buildThemeStylesInner()
    return this.cachedStyles
  }

  private cachedStyles = ''
  private cachedTheme: ThemeName = 'sepia'
  private cachedFontSize = 100

  private buildThemeStylesInner(): string {
    const theme = readerThemes[this.theme]
    const { doubleMargin, doublePadding, doubleGutter } = this.layoutSettings
    let css = ''
    for (const [selector, props] of Object.entries(theme)) {
      css += `.reader-vp ${selector.replace('body', '.reader-content')} {\n`
      for (const [prop, val] of Object.entries(props)) {
        css += `  ${prop}: ${val};\n`
      }
      css += '}\n'
    }
    for (const [selector, props] of Object.entries(baseTypography)) {
      css += `.reader-vp ${selector.replace('body', '.reader-content')} {\n`
      for (const [prop, val] of Object.entries(props)) {
        css += `  ${prop}: ${val};\n`
      }
      css += '}\n'
    }
    css += `
      .reader-vp {
        width: 100%; height: 100%;
        overflow: hidden;          /* 无滚动条：翻页是整屏替换，不是滚动 */
        position: relative;
        scroll-behavior: auto;
      }
      .reader-page-window {
        position: relative;
        z-index: 1;
        width: 100%;
        height: 100%;
        overflow: hidden;
        scroll-behavior: auto;
      }
      .reader-content {
        box-sizing: border-box;
        font-size: ${this.fontSize}%;
        width: 100%;
        height: 100%;
        padding: 16px 24px;
        max-width: 900px;
        margin: 0 auto;
        text-align: justify;
        overflow: hidden;
      }
      .reader-page {
        box-sizing: border-box;
        width: 100%;
        height: 100%;
        overflow: auto;
        scrollbar-width: none;
      }
      .reader-page::-webkit-scrollbar {
        display: none;
      }
      .reader-page > :first-child {
        margin-top: 0;
      }
      .reader-page > :last-child {
        margin-bottom: 0;
      }
      .reader-vp.reader-vp-double {
        --double-margin: ${doubleMargin}px;
        --double-padding: ${doublePadding}px;
        --double-gutter: ${doubleGutter}px;
      }
      .reader-vp.reader-vp-double::before {
        content: "";
        position: absolute;
        inset: var(--double-margin);
        pointer-events: none;
        z-index: 0;
        box-shadow: inset 0 0 0 1px var(--border);
        opacity: 0.42;
      }
      .reader-vp.reader-vp-double::after {
        content: "";
        position: absolute;
        top: var(--double-margin);
        bottom: var(--double-margin);
        left: 50%;
        width: 1px;
        pointer-events: none;
        z-index: 2;
        background: var(--border);
        opacity: 0.72;
      }
      .reader-vp.reader-vp-double .reader-page-window {
        position: absolute;
        inset: var(--double-margin);
        width: auto;
        height: auto;
      }
      .reader-vp.reader-vp-double .reader-content {
        box-sizing: border-box;
        display: grid;
        grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
        gap: var(--double-gutter);
        width: 100%;
        height: 100%;
        max-width: none;
        margin: 0;
        padding: var(--double-padding);
      }
      .reader-content img { max-width: 100%; height: auto; }
    `
    return css
  }

  private getViewport(): HTMLElement | null {
    return this.viewer?.querySelector('.reader-page-window') as HTMLElement | null
  }

  private getFrame(): HTMLElement | null {
    return this.viewer?.querySelector('.reader-vp') as HTMLElement | null
  }

  // 目录(NCX)链接常带 opf_dir 前缀，与 spine 的 manifest 原始 href 不一致，
  // 不做兜底会 idx=-1，导致换章失败、进度算错。
  private spineIndexOf(href: string): number {
    if (!this.bookInfo) return -1
    const spine = this.bookInfo.spine
    const base = href.split('#')[0]
    const exact = spine.findIndex(s => s.href.split('#')[0] === base)
    if (exact >= 0) return exact
    const name = base.split('/').pop() || base
    return spine.findIndex(s => (s.href.split('/').pop() || s.href) === name)
  }

  // ---- 标注管理 ----

  // 从当前选区创建高亮
  async addHighlightFromSelection(color: HighlightColor) {
    const anchor = computeAnchor()
    if (!anchor) return

    const ann: Annotation = {
      id: crypto.randomUUID(),
      bookId: this.bookId,
      kind: 'highlight',
      color,
      locator: { chapterHref: this.currentChapter, anchor },
      createdAt: Date.now(),
      updatedAt: Date.now(),
    }

    await saveAnnotation(ann)
    this.annotations.push(ann)
    this.onAnnotationsChanged?.(this.annotations)

    // 视觉高亮（选区还在时直接应用）
    const sel = window.getSelection()
    if (sel && !sel.isCollapsed && sel.rangeCount) {
      applyHighlight(sel.getRangeAt(0), color, ann.id)
      sel.removeAllRanges()
    } else {
      renderAnnotations(this.annotations, this.currentChapter)
    }

    return ann
  }

  // 更新标注笔记
  async updateAnnotationNote(id: string, note: string) {
    const ann = this.annotations.find(a => a.id === id)
    if (!ann) return
    ann.note = note
    ann.updatedAt = Date.now()
    await saveAnnotation(ann)
    this.onAnnotationsChanged?.(this.annotations)
  }

  // 删除标注
  async removeAnnotation(id: string) {
    await deleteAnnotation(id)
    this.annotations = this.annotations.filter(a => a.id !== id)
    // 移除视觉标记
    const el = document.querySelector(`mark[data-annotation-id="${id}"]`)
    if (el) {
      const parent = el.parentNode
      if (parent) {
        while (el.firstChild) parent.insertBefore(el.firstChild, el)
        parent.removeChild(el)
        parent.normalize()
      }
    }
    this.onAnnotationsChanged?.(this.annotations)
  }

  // bookmark：标记当前页（带章节+进度）
  async addBookmark() {
    const anchor = computeAnchor()
    if (!anchor) return
    const ann: Annotation = {
      id: crypto.randomUUID(),
      bookId: this.bookId,
      kind: 'bookmark',
      locator: { chapterHref: this.currentChapter, anchor },
      createdAt: Date.now(),
      updatedAt: Date.now(),
    }
    await saveAnnotation(ann)
    this.annotations.push(ann)
    this.onAnnotationsChanged?.(this.annotations)
  }

  getBookId() { return this.bookId }

  // 构建章节标题映射（从 TOC 扁平化）
  exportMarkdown(): string {
    const titles = new Map<string, string>()
    const flattenToc = (items: TocItem[]) => {
      for (const item of items) {
        if (item.href) titles.set(item.href, item.label)
        if (item.subitems) flattenToc(item.subitems)
      }
    }
    if (this.bookInfo) flattenToc(this.bookInfo.toc)

    const ctx: ExportContext = {
      bookTitle: this.bookInfo?.metadata.title || '未命名',
      chapterTitles: titles,
    }
    return exportAnnotations(this.annotations, ctx)
  }

  // 与 exportMarkdown 同源的章标题上下文，输出完整结构化 JSON。
  exportJson(): string {
    const titles = new Map<string, string>()
    const flattenToc = (items: TocItem[]) => {
      for (const item of items) {
        if (item.href) titles.set(item.href, item.label)
        if (item.subitems) flattenToc(item.subitems)
      }
    }
    if (this.bookInfo) flattenToc(this.bookInfo.toc)
    const ctx: ExportContext = {
      bookTitle: this.bookInfo?.metadata.title || '未命名',
      chapterTitles: titles,
    }
    return exportAnnotationsJson(this.annotations, ctx)
  }

  // ---- 滚动/翻页

  private pageTurnBusy = false
  private queuedPageDir: 1 | -1 | null = null
  private pageTurnReleaseFrame: number | null = null

  // 行高（px）：让翻页对齐整行，折页处不把一行切成两半。
  private getLineHeightPx(content: HTMLElement): number {
    const cs = getComputedStyle(content)
    const lh = parseFloat(cs.lineHeight)
    if (!Number.isNaN(lh)) return lh
    const fs = parseFloat(cs.fontSize) || 16
    return fs * 1.7 // 与 baseTypography 的 line-height 兜底一致
  }

  // 翻页 = 离散整页吸附：scrollTop 吸附到 clientHeight 的整数倍，
  // 每次 → 干脆地跳一整页（而非黏糊的部分滚动）；到章末/章首无缝跨章。
  async nextPage() {
    await this.turnPage(1)
  }

  async prevPage() {
    await this.turnPage(-1)
  }

  private async turnPage(dir: 1 | -1) {
    if (this.pageTurnBusy) {
      this.queuedPageDir = dir
      return
    }

    this.pageTurnBusy = true
    try {
      await this.performPageTurn(dir)
    } finally {
      this.pageTurnReleaseFrame = requestAnimationFrame(() => {
        this.pageTurnReleaseFrame = null
        const queued = this.queuedPageDir
        this.queuedPageDir = null
        this.pageTurnBusy = false
        if (queued) void this.turnPage(queued)
      })
    }
  }

  private async performPageTurn(dir: 1 | -1) {
    if (this.currentPages.length === 0) return
    const delta = this.visiblePageCount()
    if (dir > 0) {
      const next = this.currentPageIndex + delta
      if (next > this.lastPageIndexForMode()) {
        await this.goToAdjacentChapter(1, 'top')
        return
      }
      this.currentPageIndex = this.normalizePageIndex(next)
      this.renderCurrentPages()
      return
    }

    const prev = this.currentPageIndex - delta
    if (prev < 0) {
      await this.goToAdjacentChapter(-1, 'bottom')
      return
    }
    this.currentPageIndex = this.normalizePageIndex(prev)
    this.renderCurrentPages()
  }

  // 页内细滚（上/下方向键）：放大后一屏放不下书页时用来上下浏览，不换页。
  lineScroll(dir: 1 | -1) {
    const content = this.viewer?.querySelector('.reader-content') as HTMLElement | null
    const page = this.viewer?.querySelector('.reader-page') as HTMLElement | null
    if (!content || !page) return
    const line = this.getLineHeightPx(content)
    page.scrollTo({ top: page.scrollTop + dir * line * 3, behavior: 'auto' })
  }

  // 跨章：dir=+1 下一章落顶部，dir=-1 上一章落底部；到书首/书尾则不动。
  private async goToAdjacentChapter(dir: 1 | -1, land: 'top' | 'bottom') {
    if (!this.bookInfo) return
    const spine = this.bookInfo.spine
    const idx = this.spineIndexOf(this.currentChapter)
    const target = idx + dir
    if (idx < 0 || target < 0 || target >= spine.length) return
    await this.loadChapter(spine[target].href)
    if (land === 'bottom') {
      this.currentPageIndex = this.lastPageIndexForMode()
      this.renderCurrentPages()
    }
  }

  private emitLocation() {
    if (!this.bookInfo || !this.viewer) return
    const spineIdx = this.spineIndexOf(this.currentChapter)
    const totalChapters = this.bookInfo.spine.length
    const chapterProgress = this.getChapterProgress()
    const base = totalChapters > 0 ? Math.max(0, spineIdx) / totalChapters : 0
    const pctPerChapter = totalChapters > 0 ? 1 / totalChapters : 1
    const percentage = base + chapterProgress * pctPerChapter

    this.onRelocated?.({
      percentage: Math.min(1, Math.max(0, percentage)),
      href: this.currentChapter
    })
    this.scheduleProgressSave(Math.min(1, Math.max(0, percentage)))
  }

  // 进度保存:防抖 800ms,翻页风暴只落一次库。
  private progressSaveTimer: number | null = null
  private lastEmittedPercentage = 0

  private scheduleProgressSave(percentage: number) {
    if (!this.bookId || !this.currentChapter) return
    this.lastEmittedPercentage = percentage
    if (this.progressSaveTimer !== null) window.clearTimeout(this.progressSaveTimer)
    this.progressSaveTimer = window.setTimeout(() => {
      this.progressSaveTimer = null
      this.persistProgress(percentage)
    }, 800)
  }

  // 换书/销毁前冲刷:把防抖中悬着的最后一次进度立即落库(此时旧书状态仍在)。
  private flushProgressSave() {
    if (this.progressSaveTimer === null) return
    window.clearTimeout(this.progressSaveTimer)
    this.progressSaveTimer = null
    this.persistProgress(this.lastEmittedPercentage)
  }

  private persistProgress(percentage: number) {
    if (!this.bookId || !this.currentChapter) return
    void bridge.saveProgress({
      bookId: this.bookId,
      chapterHref: this.currentChapter,
      chapterProgress: this.getChapterProgress(),
      percentage,
      updatedAt: Date.now(),
    }).catch(() => {})
  }

  private scheduleEmitLocation() {
    if (this.locationFrame !== null) return
    this.locationFrame = requestAnimationFrame(() => {
      this.locationFrame = null
      this.emitLocation()
    })
  }

  setTheme(name: ThemeName) {
    this.theme = name
    this.cachedStyles = ''
    const styleEl = this.viewer?.querySelector('#reader-styles')
    if (styleEl) styleEl.textContent = this.buildThemeStyles()
    if (this.viewer && this.currentChapter) this.repaginateCurrentChapter()
  }

  setPageMode(mode: PageMode) {
    if (this.pageMode === mode) return
    const progress = this.getChapterProgress()
    this.pageMode = mode
    const vp = this.getViewport()
    if (vp) {
      this.getFrame()?.classList.toggle('reader-vp-double', mode === 'double')
      this.cachedStyles = ''
      const styleEl = this.viewer?.querySelector('#reader-styles')
      if (styleEl) styleEl.textContent = this.buildThemeStyles()
      this.currentPages = this.getOrBuildPageModel(this.currentChapterBase, this.currentChapterHtml)
      this.setChapterProgress(progress)
    }
    this.emitLocation()
  }

  getPageMode() { return this.pageMode }

  setLayoutSettings(settings: Partial<ReaderLayoutSettings>) {
    const progress = this.getChapterProgress()
    this.layoutSettings = {
      doubleMargin: this.clampNumber(settings.doubleMargin ?? this.layoutSettings.doubleMargin, 12, 160),
      doublePadding: this.clampNumber(settings.doublePadding ?? this.layoutSettings.doublePadding, 16, 120),
      doubleGutter: this.clampNumber(settings.doubleGutter ?? this.layoutSettings.doubleGutter, 40, 220),
    }
    this.cachedStyles = ''
    const styleEl = this.viewer?.querySelector('#reader-styles')
    if (styleEl) styleEl.textContent = this.buildThemeStyles()
    if (this.currentChapterHtml) {
      this.currentPages = this.getOrBuildPageModel(this.currentChapterBase, this.currentChapterHtml)
      this.setChapterProgress(progress)
    }
  }

  getLayoutSettings(): ReaderLayoutSettings {
    return { ...this.layoutSettings }
  }

  private clampNumber(value: number, min: number, max: number) {
    if (!Number.isFinite(value)) return min
    return Math.max(min, Math.min(max, value))
  }

  private getChapterProgress(): number {
    const max = Math.max(0, this.currentPages.length - 1)
    if (max <= 0) return 0
    return Math.min(1, Math.max(0, this.currentPageIndex / max))
  }

  private setChapterProgress(progress: number) {
    const p = Math.min(1, Math.max(0, progress))
    const max = Math.max(0, this.currentPages.length - 1)
    this.currentPageIndex = this.normalizePageIndex(p * max)
    this.renderCurrentPages()
    this.emitLocation()
  }

  setFontSize(percent: number) {
    this.cachedStyles = ''
    const prevRatio = this.getChapterProgress()
    this.fontSize = Math.max(50, Math.min(300, percent))
    if (this.viewer && this.currentChapterHtml) {
      const styleEl = this.viewer.querySelector('#reader-styles')
      if (styleEl) styleEl.textContent = this.buildThemeStyles()
      this.currentPages = this.getOrBuildPageModel(this.currentChapterBase, this.currentChapterHtml)
      this.setChapterProgress(prevRatio)
    }
  }

  getFontSize() { return this.fontSize }

  private resetRuntimeCaches() {
    this.flushProgressSave()
    this.chapterLoadSeq++
    this.currentChapter = ''
    this.currentChapterBase = ''
    this.currentChapterHtml = ''
    this.currentPages = []
    this.currentPageIndex = 0
    this.chapterCache.clear()
    this.chapterInflight.clear()
    this.pageModelCache.clear()
    if (this.locationFrame !== null) {
      cancelAnimationFrame(this.locationFrame)
      this.locationFrame = null
    }
    if (this.pageTurnReleaseFrame !== null) {
      cancelAnimationFrame(this.pageTurnReleaseFrame)
      this.pageTurnReleaseFrame = null
    }
    if (this.repaginateFrame !== null) {
      cancelAnimationFrame(this.repaginateFrame)
      this.repaginateFrame = null
    }
    this.pageTurnBusy = false
    this.queuedPageDir = null
  }

  destroy() {
    this.flushProgressSave()
    this.bookInfo = null
    this.bookId = ''
    this.annotations = []
    this.resetRuntimeCaches()
    this.resizeObserver?.disconnect()
    this.resizeObserver = null
    if (this.viewer) this.viewer.innerHTML = ''
    this.viewer = null
    bridge.closeBook().catch(() => {})
  }
}

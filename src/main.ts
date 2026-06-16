import { ReaderCore, type PageMode, type ReaderLayoutSettings, type TocItem } from './reader-core'
import { bridge, hasNativeBridge, type LibraryBook } from './platform'
import type { ThemeName } from './themes'

const reader = new ReaderCore()

const $ = <T extends HTMLElement = HTMLElement>(sel: string) =>
  document.querySelector(sel) as T

const viewer = $('#viewer')
const emptyState = $('#empty-state')
const statusbar = $('#statusbar')
const sidebar = $('#sidebar')
const tocEl = $('#toc')
const bookTitle = $('#book-title')
const progressRange = $<HTMLInputElement>('#progress-range')
const progressLabel = $('#progress-label')
const fileInput = $<HTMLInputElement>('#file-input')
const pageModeBtn = $('#btn-page-mode')
const layoutBtn = $('#btn-layout')
const layoutPopover = $('#layout-popover')
const layoutMarginInput = $<HTMLInputElement>('#layout-margin')
const layoutPaddingInput = $<HTMLInputElement>('#layout-padding')
const layoutGutterInput = $<HTMLInputElement>('#layout-gutter')
const layoutMarginValue = $('#layout-margin-value')
const layoutPaddingValue = $('#layout-padding-value')
const layoutGutterValue = $('#layout-gutter-value')
const isTauriRuntime = hasNativeBridge

// 防止程序更新进度条时触发 loadChapter
let updatingProgress = false
let statusbarHideTimer: number | null = null
let activeTocBase = ''
let pendingSeekPct: number | null = null
let layoutFrame: number | null = null

function setStatusbarVisible(visible: boolean) {
  document.body.classList.toggle('statusbar-visible', visible && !statusbar.hidden)
}

function revealStatusbar() {
  if (statusbar.hidden) return
  setStatusbarVisible(true)
  if (statusbarHideTimer !== null) window.clearTimeout(statusbarHideTimer)
  statusbarHideTimer = window.setTimeout(() => {
    if (!statusbar.matches(':hover') && !statusbar.matches(':focus-within')) {
      setStatusbarVisible(false)
    }
  }, 1200)
}

document.addEventListener('pointermove', (e) => {
  if (statusbar.hidden) return
  if (window.innerHeight - e.clientY < 90) {
    revealStatusbar()
  }
})
statusbar.addEventListener('mouseleave', () => {
  if (!statusbar.matches(':focus-within')) setStatusbarVisible(false)
})

function showError(msg: string) {
  console.error(msg)
  const p = emptyState.querySelector('p')
  if (p) p.textContent = `错误: ${msg}`
}

$('#btn-open').addEventListener('click', () => fileInput.click())
$('#btn-empty-open')?.addEventListener('click', () => fileInput.click())
$('#btn-empty-library')?.addEventListener('click', openLibrary)

fileInput.addEventListener('change', async () => {
  const file = fileInput.files?.[0]
  if (!file) return
  await openBook(file)
  fileInput.value = ''
})

async function openBook(file: File) {
  if (!isTauriRuntime()) {
    showError('打开 EPUB 需要 Tauri 桌面窗口。请在终端运行：pnpm tauri dev')
    return
  }
  try {
    const buf = await file.arrayBuffer()
    await reader.openFromBuffer(buf, viewer)
    emptyState.hidden = true
    statusbar.hidden = false
    $('#prev-zone').hidden = false
    $('#next-zone').hidden = false
    document.body.classList.add('reading-active')
  } catch (err: any) {
    showError(`解析 EPUB 失败: ${err.message || err}`)
  }
}

reader.onReady = () => {
  const info = reader.bookInfo!
  activeTocBase = ''
  pendingSeekPct = null
  bookTitle.textContent = info.metadata.title || '未命名'
  renderToc(info.toc)
}

reader.onRelocated = (info) => {
  updatingProgress = true
  const pct = Math.round(info.percentage * 1000) / 10
  progressRange.value = String(pct)
  progressLabel.textContent = `${pct}%`
  updatingProgress = false
  const base = info.href.split('#')[0]
  if (base !== activeTocBase) {
    activeTocBase = base
    highlightToc(info.href)
  }
}

// —— 目录渲染（用 button 替代 a，确保跨 WebView 可点击）——
function renderToc(toc: TocItem[]) {
  tocEl.innerHTML = ''
  const addItem = (item: TocItem, indent: number) => {
    const hasHref = item.href && item.href.trim().length > 0
    const hasLabel = item.label && item.label.trim().length > 0

    if (hasHref && hasLabel) {
      const btn = document.createElement('button')
      btn.textContent = item.label.trim()
      btn.style.display = 'block'
      btn.style.width = '100%'
      btn.style.textAlign = 'left'
      btn.style.padding = '7px 16px'
      btn.style.paddingLeft = `${16 + indent * 14}px`
      btn.style.background = 'transparent'
      btn.style.border = 'none'
      btn.style.borderLeft = '3px solid transparent'
      btn.style.color = 'var(--fg)'
      btn.style.fontSize = '13px'
      btn.style.cursor = 'pointer'
      btn.style.lineHeight = '1.4'
      btn.dataset.href = item.href
      btn.addEventListener('click', () => {
        reader.loadChapter(item.href).catch((err: any) => {
          showError(`加载章节失败: ${err.message || err}`)
        })
      })
      btn.addEventListener('mouseenter', () => {
        btn.style.background = 'var(--bg)'
      })
      btn.addEventListener('mouseleave', () => {
        btn.style.background = 'transparent'
      })
      tocEl.appendChild(btn)
    } else if (hasLabel) {
      const div = document.createElement('div')
      div.textContent = item.label.trim()
      div.style.paddingLeft = `${16 + indent * 14}px`
      div.style.fontWeight = '600'
      div.style.fontSize = '13px'
      div.style.color = 'var(--muted)'
      div.style.paddingTop = '8px'
      div.style.paddingBottom = '4px'
      tocEl.appendChild(div)
    }

    const childIndent = hasLabel || hasHref ? indent + 1 : indent
    item.subitems?.forEach(sub => addItem(sub, childIndent))
  }
  toc.forEach(item => addItem(item, 0))
}

function highlightToc(href: string) {
  if (!href) return
  const base = href.split('#')[0]
  tocEl.querySelectorAll('button[data-href]').forEach(btn => {
    const b = btn as HTMLButtonElement
    const h = b.dataset.href || ''
    b.style.borderLeftColor = h.split('#')[0] === base ? 'var(--accent)' : 'transparent'
    b.style.color = h.split('#')[0] === base ? 'var(--accent)' : 'var(--fg)'
  })
}

$('#btn-toc').addEventListener('click', () => {
  sidebar.hidden = !sidebar.hidden
})

// —— 翻页 ——
function doNextPage() {
  reader.nextPage()
}
function doPrevPage() {
  reader.prevPage()
}
$('#btn-next').addEventListener('click', doNextPage)
$('#btn-prev').addEventListener('click', doPrevPage)
$('#next-zone').addEventListener('click', doNextPage)
$('#prev-zone').addEventListener('click', doPrevPage)
document.addEventListener('keydown', (e) => {
  // 只在"真正的文本录入控件"里放行原生按键（将来的笔记输入框）。
  // range 滑块等非文本控件不放行——否则方向键会被进度条当作调值/跳章吞掉，无法翻页。
  const el = e.target as HTMLElement
  const isTextEntry =
    el?.tagName === 'TEXTAREA' ||
    (el?.tagName === 'INPUT' &&
      !['range', 'checkbox', 'radio', 'button', 'submit'].includes((el as HTMLInputElement).type))
  if (isTextEntry) return
  switch (e.key) {
    // 左右 = 换页（上一页/下一页），到章末/章首自动跨章
    case 'ArrowLeft':
    case 'PageUp':
      e.preventDefault(); reader.prevPage(); break
    case 'ArrowRight':
    case 'PageDown':
    case ' ':
      e.preventDefault(); reader.nextPage(); break
    // 上下 = 页内细滚（放大后一屏放不下时浏览），不换页
    case 'ArrowUp':
      e.preventDefault(); reader.lineScroll(-1); break
    case 'ArrowDown':
      e.preventDefault(); reader.lineScroll(1); break
  }
})

// —— 进度条：拖动时只预览百分比，松手后只跳一次，避免连续整章重排卡死 ——
progressRange.addEventListener('input', () => {
  if (updatingProgress) return
  const pct = Number(progressRange.value)
  pendingSeekPct = pct
  progressLabel.textContent = `${Math.round(pct * 10) / 10}%`
})

function commitProgressSeek() {
  if (updatingProgress || pendingSeekPct === null) return
  const pct = pendingSeekPct / 100
  pendingSeekPct = null
  const info = reader.bookInfo
  if (!info) return
  const idx = Math.floor(pct * info.spine.length)
  const clamped = Math.min(info.spine.length - 1, Math.max(0, idx))
  reader.loadChapter(info.spine[clamped].href).catch((err: any) => {
    showError(`跳转失败: ${err.message || err}`)
  })
}

progressRange.addEventListener('pointerup', commitProgressSeek)
progressRange.addEventListener('keyup', (e) => {
  if (['ArrowLeft', 'ArrowRight', 'Home', 'End', 'PageUp', 'PageDown'].includes(e.key)) {
    commitProgressSeek()
  }
})
// 拖动定位完成后让滑块失焦，焦点回到正文，方向键即正常翻页
progressRange.addEventListener('change', () => {
  commitProgressSeek()
  progressRange.blur()
})

// —— 字号 ——
$('#btn-font-inc').addEventListener('click', () => reader.setFontSize(reader.getFontSize() + 10))
$('#btn-font-dec').addEventListener('click', () => reader.setFontSize(reader.getFontSize() - 10))

// —— 单页/双页 ——
const PAGE_MODE_KEY = 'reader-page-mode'
function applyPageMode(mode: PageMode) {
  reader.setPageMode(mode)
  pageModeBtn.textContent = mode === 'double' ? '双页' : '单页'
  pageModeBtn.title = mode === 'double' ? '当前为双页，点击切换单页' : '当前为单页，点击切换双页'
  pageModeBtn.classList.toggle('active', mode === 'double')
  localStorage.setItem(PAGE_MODE_KEY, mode)
}
pageModeBtn.addEventListener('click', () => {
  applyPageMode(reader.getPageMode() === 'double' ? 'single' : 'double')
})
applyPageMode((localStorage.getItem(PAGE_MODE_KEY) as PageMode) || 'single')

// —— 版式 ——
const LAYOUT_KEY = 'reader-layout-settings'
const DEFAULT_LAYOUT: ReaderLayoutSettings = {
  doubleMargin: 84,
  doublePadding: 42,
  doubleGutter: 108,
}
let currentLayout: ReaderLayoutSettings = readLayoutSettings()

function readLayoutSettings(): ReaderLayoutSettings {
  try {
    const raw = localStorage.getItem(LAYOUT_KEY)
    if (!raw) return { ...DEFAULT_LAYOUT }
    const parsed = JSON.parse(raw) as Partial<ReaderLayoutSettings>
    return {
      doubleMargin: Number(parsed.doubleMargin ?? DEFAULT_LAYOUT.doubleMargin),
      doublePadding: Number(parsed.doublePadding ?? DEFAULT_LAYOUT.doublePadding),
      doubleGutter: Number(parsed.doubleGutter ?? DEFAULT_LAYOUT.doubleGutter),
    }
  } catch {
    return { ...DEFAULT_LAYOUT }
  }
}

function writeLayoutInputs(settings: ReaderLayoutSettings) {
  layoutMarginInput.value = String(settings.doubleMargin)
  layoutPaddingInput.value = String(settings.doublePadding)
  layoutGutterInput.value = String(settings.doubleGutter)
  layoutMarginValue.textContent = `${settings.doubleMargin}px`
  layoutPaddingValue.textContent = `${settings.doublePadding}px`
  layoutGutterValue.textContent = `${settings.doubleGutter}px`
}

function applyLayoutSettings(settings: ReaderLayoutSettings) {
  reader.setLayoutSettings({
    doubleMargin: Number(settings.doubleMargin),
    doublePadding: Number(settings.doublePadding),
    doubleGutter: Number(settings.doubleGutter),
  })
  currentLayout = reader.getLayoutSettings()
  writeLayoutInputs(currentLayout)
  localStorage.setItem(LAYOUT_KEY, JSON.stringify(currentLayout))
}

function readLayoutInputs(): ReaderLayoutSettings {
  return {
    doubleMargin: Number(layoutMarginInput.value),
    doublePadding: Number(layoutPaddingInput.value),
    doubleGutter: Number(layoutGutterInput.value),
  }
}

function onLayoutInput() {
  writeLayoutInputs(readLayoutInputs())
  if (layoutFrame !== null) return
  layoutFrame = requestAnimationFrame(() => {
    layoutFrame = null
    applyLayoutSettings(readLayoutInputs())
  })
}

layoutBtn.addEventListener('click', (e) => {
  e.stopPropagation()
  layoutPopover.hidden = !layoutPopover.hidden
  layoutBtn.classList.toggle('active', !layoutPopover.hidden)
})
layoutPopover.addEventListener('click', (e) => e.stopPropagation())
layoutPopover.addEventListener('keydown', (e) => e.stopPropagation())
document.addEventListener('click', () => {
  layoutPopover.hidden = true
  layoutBtn.classList.remove('active')
})
layoutMarginInput.addEventListener('input', onLayoutInput)
layoutPaddingInput.addEventListener('input', onLayoutInput)
layoutGutterInput.addEventListener('input', onLayoutInput)
$('#btn-layout-reset').addEventListener('click', () => applyLayoutSettings({ ...DEFAULT_LAYOUT }))
writeLayoutInputs(currentLayout)
applyLayoutSettings(currentLayout)

// —— 主题 ——
const THEME_KEY = 'reader-theme'
function applyTheme(name: ThemeName) {
  document.body.dataset.theme = name
  reader.setTheme(name)
  localStorage.setItem(THEME_KEY, name)
  document.querySelectorAll<HTMLElement>('.theme-btn').forEach(btn => {
    btn.classList.toggle('active', btn.dataset.theme === name)
  })
}
document.querySelectorAll<HTMLElement>('.theme-btn').forEach(btn => {
  btn.addEventListener('click', () => applyTheme(btn.dataset.theme as ThemeName))
})
applyTheme((localStorage.getItem(THEME_KEY) as ThemeName) || 'light')

// —— 标注系统 ——
import { HIGHLIGHT_COLORS, type HighlightColor } from './annotations'

const annBtn = $('#btn-annotations')
const annSidebar = $('#ann-sidebar')
const annList = $('#ann-list')

// 侧栏开关
annBtn?.addEventListener('click', () => {
  annSidebar.hidden = !annSidebar.hidden
  if (!annSidebar.hidden) renderAnnList()
})

// 文本选中 → 弹出高亮色盘
let colorPopup: HTMLElement | null = null
document.addEventListener('mouseup', (e) => {
  setTimeout(() => {
    const sel = window.getSelection()
    if (!sel || sel.isCollapsed) {
      colorPopup?.remove(); colorPopup = null
      return
    }
    const text = sel.toString().trim()
    if (text.length === 0) {
      colorPopup?.remove(); colorPopup = null
      return
    }
    // 只在正文区显示
    if (!(e.target as HTMLElement).closest('.reader-content')) return
    showColorPopup(e.clientX, e.clientY)
  }, 10)
})

function showColorPopup(x: number, y: number) {
  colorPopup?.remove()
  const popup = document.createElement('div')
  popup.className = 'color-popup'
  popup.style.cssText = `
    position:fixed; z-index:999; left:${x}px; top:${y-36}px;
    display:flex; gap:4px; background:var(--panel,#1e1e1e);
    border:1px solid var(--border,#333); border-radius:8px; padding:6px;
    box-shadow:0 4px 16px rgba(0,0,0,.4);
  `
  for (const [color, palette] of Object.entries(HIGHLIGHT_COLORS)) {
    const btn = document.createElement('button')
    btn.style.cssText = `
      width:28px;height:28px;border-radius:6px;border:2px solid ${palette.border};
      background:${palette.bg};cursor:pointer;
    `
    btn.title = color
    btn.addEventListener('mousedown', (e) => {
      e.preventDefault()
      reader.addHighlightFromSelection(color as HighlightColor)
      popup.remove(); colorPopup = null
    })
    popup.appendChild(btn)
  }
  document.body.appendChild(popup)
  colorPopup = popup
  // 3 秒后自动消失
  setTimeout(() => { popup.remove(); colorPopup = null }, 3000)
}

// 点击已有高亮 → 显示批注/删除
viewer.addEventListener('click', (e) => {
  const mark = (e.target as HTMLElement).closest('mark[data-annotation-id]') as HTMLElement | null
  if (!mark) return
  const id = mark.dataset.annotationId!
  const ann = reader.annotations.find(a => a.id === id)
  if (!ann) return
  showAnnDetail(e.clientX, e.clientY, ann)
})

function showAnnDetail(x: number, y: number, ann: import('./annotations').Annotation) {
  const popup = document.createElement('div')
  popup.className = 'ann-detail'
  const color = ann.color || 'yellow'
  const palette = HIGHLIGHT_COLORS[color as HighlightColor] || HIGHLIGHT_COLORS.yellow
  popup.style.cssText = `
    position:fixed; z-index:999; left:${x}px; top:${y-12}px;
    background:var(--panel,#1e1e1e); border:1px solid var(--border,#333);
    border-left:4px solid ${palette.border};
    border-radius:8px; padding:12px; max-width:280px;
    box-shadow:0 4px 16px rgba(0,0,0,.4);font-size:13px;
  `
  popup.innerHTML = `
    <div style="margin-bottom:6px;color:var(--muted,#888)">${ann.locator.anchor.exact || '(无文本)'}</div>
    <textarea id="ann-note-input" placeholder="添加批注..." style="
      width:100%;min-height:50px;background:var(--bg,#111);color:var(--fg,#ddd);
      border:1px solid var(--border,#333);border-radius:4px;padding:6px;
      font-size:12px;resize:vertical;margin-bottom:8px;
    ">${ann.note || ''}</textarea>
    <div style="display:flex;gap:6px;justify-content:flex-end">
      <button id="ann-save" style="padding:4px 12px;border-radius:4px;border:1px solid var(--accent,#f90);background:var(--accent,#f90);color:#000;cursor:pointer;font-size:12px;">保存</button>
      <button id="ann-del" style="padding:4px 12px;border-radius:4px;border:1px solid var(--border,#333);background:transparent;color:var(--fg,#ddd);cursor:pointer;font-size:12px;">删除</button>
    </div>
  `
  document.body.appendChild(popup)

  popup.querySelector('#ann-save')?.addEventListener('click', () => {
    const note = (popup.querySelector('#ann-note-input') as HTMLTextAreaElement)?.value || ''
    reader.updateAnnotationNote(ann.id, note)
    popup.remove()
  })
  popup.querySelector('#ann-del')?.addEventListener('click', () => {
    reader.removeAnnotation(ann.id)
    popup.remove()
  })
  // 点击外部关闭
  setTimeout(() => {
    const close = (ev: MouseEvent) => {
      if (!popup.contains(ev.target as Node)) { popup.remove(); document.removeEventListener('click', close) }
    }
    document.addEventListener('click', close)
  }, 0)
}

// 标注列表
reader.onAnnotationsChanged = () => renderAnnList()

function renderAnnList() {
  if (!annList) return
  const anns = reader.annotations
  annList.innerHTML = anns.length === 0
    ? '<div style="padding:24px;text-align:center;color:var(--muted,#888);font-size:13px">暂无标注</div>'
    : anns.map(a => {
        const color = a.color || 'yellow'
        const palette = HIGHLIGHT_COLORS[color as HighlightColor]
        return `
          <div class="ann-item" data-id="${a.id}" style="
            padding:10px 16px;border-bottom:1px solid var(--border,#333);
            border-left:3px solid ${palette?.border || '#888'};cursor:pointer;
            font-size:13px;
          ">
            <div style="color:var(--muted,#888);font-size:11px;margin-bottom:2px">
              ${a.kind === 'bookmark' ? '📑 书签' : '🖍️ 高亮'} · ${new Date(a.createdAt).toLocaleString('zh-CN')}
            </div>
            <div style="margin-bottom:2px;line-height:1.4">${a.locator.anchor.exact?.substring(0, 80) || '(无文本)'}</div>
            ${a.note ? `<div style="color:var(--accent,#f90);font-size:12px;font-style:italic">💬 ${a.note.substring(0, 60)}</div>` : ''}
          </div>`
      }).join('')

  // 点击跳转
  annList.querySelectorAll('.ann-item').forEach(el => {
    el.addEventListener('click', () => {
      const id = (el as HTMLElement).dataset.id!
      const ann = reader.annotations.find(a => a.id === id)
      if (ann) reader.loadChapter(ann.locator.chapterHref)
    })
  })
}

// —— 导出 Markdown ——
$('#btn-export')?.addEventListener('click', () => {
  const md = reader.exportMarkdown()
  if (!md) return
  const blob = new Blob(['\uFEFF' + md], { type: 'text/markdown;charset=utf-8' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = `${reader.bookInfo?.metadata.title || '标注导出'}_标注.md`
  a.click()
  URL.revokeObjectURL(url)
})

// —— 导出标注 JSON（完整结构化数据，可还原）——
$('#btn-export-json')?.addEventListener('click', () => {
  if (!reader.bookInfo) return
  const json = reader.exportJson()
  const title = reader.bookInfo.metadata.title || '标注导出'
  const blob = new Blob([json], { type: 'application/json;charset=utf-8' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = `${title}_标注.json`
  a.click()
  URL.revokeObjectURL(url)
})

// —— 本地书库 + Calibre 导入来源 ——
const LIBRARY_PATH_KEY = 'reader.calibreImportPath'
const DEFAULT_CALIBRE_LIBRARY = 'F:\\Calibre书库'
const libraryView = $('#library-view')
const libraryGrid = $('#library-grid')
const libraryPathInput = $<HTMLInputElement>('#library-path-input')
const libraryImportInput = $<HTMLInputElement>('#library-import-input')
const libraryFolderInput = $<HTMLInputElement>('#library-folder-input')
const librarySearchInput = $<HTMLInputElement>('#library-search-input')
const librarySourcePanel = $<HTMLDetailsElement>('#library-source-panel')
let libraryBooks: LibraryBook[] = []
let librarySearchTimer: number | null = null
libraryPathInput.value = localStorage.getItem(LIBRARY_PATH_KEY) || DEFAULT_CALIBRE_LIBRARY

$('#btn-library')?.addEventListener('click', openLibrary)
$('#btn-library-close')?.addEventListener('click', () => { libraryView.hidden = true })
$('#btn-library-refresh')?.addEventListener('click', refreshLibraryBooks)
$('#btn-library-import-epub')?.addEventListener('click', () => libraryImportInput.click())
$('#btn-library-import-folder')?.addEventListener('click', () => libraryFolderInput.click())
$('#btn-library-import-calibre')?.addEventListener('click', importCalibreLibrary)
libraryImportInput.addEventListener('change', async () => {
  const files = collectEpubFiles(Array.from(libraryImportInput.files || []))
  libraryImportInput.value = ''
  await importEpubFiles(files, 'EPUB')
})
libraryFolderInput.addEventListener('change', async () => {
  const files = collectEpubFiles(Array.from(libraryFolderInput.files || []))
  libraryFolderInput.value = ''
  await importEpubFiles(files, '文件夹')
})
libraryPathInput.addEventListener('keydown', (e) => {
  if (e.key === 'Enter') importCalibreLibrary()
})
librarySearchInput.addEventListener('input', () => {
  if (librarySearchTimer !== null) window.clearTimeout(librarySearchTimer)
  librarySearchTimer = window.setTimeout(() => {
    librarySearchTimer = null
    refreshLibraryBooks()
  }, 180)
})

async function openLibrary() {
  libraryView.hidden = false
  await refreshLibraryBooks()
}

async function refreshLibraryBooks() {
  libraryView.hidden = false
  libraryGrid.innerHTML = '<div class="library-state">加载书库中…</div>'
  if (!isTauriRuntime()) {
    libraryGrid.innerHTML = '<div class="library-state">书库读取需要 Tauri 桌面窗口。<br>请在终端运行：<code>npm run tauri dev</code></div>'
    return
  }
  try {
    const query = librarySearchInput.value.trim()
    libraryBooks = query
      ? await bridge.searchLibraryBooks(query)
      : await bridge.listLibraryBooks()
    renderLibraryBooks()
  } catch (e: any) {
    libraryGrid.innerHTML = `<div class="library-state library-state-error">读取书库失败：${e?.message || e}</div>`
  }
}

async function importCalibreLibrary() {
  const library = libraryPathInput.value.trim() || DEFAULT_CALIBRE_LIBRARY
  libraryPathInput.value = library
  localStorage.setItem(LIBRARY_PATH_KEY, library)
  libraryView.hidden = false
  libraryGrid.innerHTML = '<div class="library-state">扫描 Calibre 迁移来源中…</div>'
  if (!isTauriRuntime()) {
    libraryGrid.innerHTML = '<div class="library-state">书库导入需要 Tauri 桌面窗口。<br>请在终端运行：<code>npm run tauri dev</code></div>'
    return
  }
  try {
    const calibreBooks = await bridge.listCalibreBooks(library)
    if (calibreBooks.length === 0) {
      libraryGrid.innerHTML = '<div class="library-state">Calibre 迁移来源里没有 EPUB</div>'
      return
    }
    let imported = 0
    let duplicate = 0
    const failedItems: string[] = []
    for (let i = 0; i < calibreBooks.length; i++) {
      const b = calibreBooks[i]
      libraryGrid.innerHTML = `<div class="library-state">迁移中 ${i + 1}/${calibreBooks.length}：${b.title || b.path}</div>`
      try {
        const result = await bridge.importLibraryBook(b.path)
        if (result.duplicate) duplicate += 1
        else imported += 1
      } catch (err) {
        failedItems.push(`${b.title || b.path}: ${formatError(err)}`)
        console.warn('导入失败', b.path, err)
      }
    }
    await refreshLibraryBooks()
    prependLibraryImportSummary(imported, duplicate, failedItems)
  } catch (e: any) {
    libraryGrid.innerHTML = `<div class="library-state library-state-error">导入失败：${e?.message || e}</div>`
  }
}

function collectEpubFiles(files: File[]): File[] {
  return files
    .filter((file) => file.name.toLowerCase().endsWith('.epub'))
    .sort((a, b) => {
      const aPath = (a as File & { webkitRelativePath?: string }).webkitRelativePath || a.name
      const bPath = (b as File & { webkitRelativePath?: string }).webkitRelativePath || b.name
      return aPath.localeCompare(bPath, 'zh-CN')
    })
}

async function importEpubFiles(files: File[], sourceLabel: string) {
  libraryView.hidden = false
  if (files.length === 0) {
    libraryGrid.innerHTML = `<div class="library-state">${sourceLabel} 中没有可导入的 EPUB</div>`
    return
  }
  if (!isTauriRuntime()) {
    libraryGrid.innerHTML = '<div class="library-state">书库导入需要 Tauri 桌面窗口。<br>请在终端运行：<code>npm run tauri dev</code></div>'
    return
  }

  let imported = 0
  let duplicate = 0
  const failedItems: string[] = []
  for (let i = 0; i < files.length; i++) {
    const file = files[i]
    const path = (file as File & { webkitRelativePath?: string }).webkitRelativePath || file.name
    libraryGrid.innerHTML = `<div class="library-state">导入中 ${i + 1}/${files.length}：${path}</div>`
    try {
      const data = new Uint8Array(await file.arrayBuffer())
      const result = await bridge.importLibraryBookFromBytes(data, file.name)
      if (result.duplicate) duplicate += 1
      else imported += 1
    } catch (err) {
      failedItems.push(`${path}: ${formatError(err)}`)
      console.warn('导入失败', path, err)
    }
  }

  await refreshLibraryBooks()
  prependLibraryImportSummary(imported, duplicate, failedItems)
}

function formatError(err: unknown): string {
  if (err instanceof Error) return err.message
  if (typeof err === 'string') return err
  return String(err)
}

function prependLibraryImportSummary(
  imported: number,
  duplicate: number,
  failedItems: string[],
) {
  const failed = failedItems.length
  if (imported === 0 && duplicate === 0 && failed === 0) return
  const msg = document.createElement('div')
  msg.className = `library-import-summary${failed > 0 ? ' error' : ''}`
  const summary = document.createElement('div')
  summary.textContent = failed > 0
    ? `导入完成：新增 ${imported} 本，已存在 ${duplicate} 本，失败 ${failed} 本`
    : `导入完成：新增 ${imported} 本，已存在 ${duplicate} 本`
  msg.appendChild(summary)
  if (failedItems.length > 0) {
    const details = document.createElement('details')
    const label = document.createElement('summary')
    label.textContent = '查看失败条目'
    const list = document.createElement('ul')
    for (const item of failedItems.slice(0, 20)) {
      const li = document.createElement('li')
      li.textContent = item
      list.appendChild(li)
    }
    if (failedItems.length > 20) {
      const li = document.createElement('li')
      li.textContent = `还有 ${failedItems.length - 20} 条失败未显示`
      list.appendChild(li)
    }
    details.append(label, list)
    msg.appendChild(details)
  }
  libraryGrid.prepend(msg)
}

function formatSeriesLabel(book: LibraryBook): string {
  if (!book.series) return ''
  if (book.seriesIndex === undefined || book.seriesIndex === null) return book.series
  const index = Number.isInteger(book.seriesIndex)
    ? String(book.seriesIndex)
    : String(book.seriesIndex).replace(/\.0+$/, '')
  return `${book.series} #${index}`
}

function formatLanguageLabel(language?: string): string {
  if (!language) return ''
  const normalized = language.trim()
  return normalized.length <= 3 ? normalized.toUpperCase() : normalized
}

function renderLibraryBooks() {
  const books = libraryBooks

  if (books.length === 0) {
    if (librarySearchInput.value.trim()) {
      libraryGrid.innerHTML = '<div class="library-state">没有匹配的书</div>'
    } else {
      renderLibraryEmptyState()
    }
    return
  }

  libraryGrid.innerHTML = ''
  for (const b of books) {
    const card = document.createElement('div')
    card.className = 'book-card'
    const cover = document.createElement('div')
    cover.className = 'cover'
    // 书架优先加载缩略图（小图省内存、滚动更流畅），无则回退原封面
    const coverSrc = b.thumbPath || b.coverPath
    if (coverSrc) {
      const img = document.createElement('img')
      img.loading = 'lazy'         // 视口外延迟加载，大书架不一次性拉全部封面
      img.decoding = 'async'
      img.src = bridge.resolveFileUrl(coverSrc)
      img.alt = ''
      cover.appendChild(img)
    } else {
      cover.textContent = '无封面'
    }
    const t = document.createElement('div'); t.className = 'title'; t.textContent = b.title || '未命名'
    const a = document.createElement('div'); a.className = 'author'; a.textContent = b.author || '佚名'
    const series = formatSeriesLabel(b)
    const language = formatLanguageLabel(b.language)
    const tags = document.createElement('div')
    tags.className = 'book-tags'
    for (const value of [series, language].filter(Boolean)) {
      const tag = document.createElement('span')
      tag.textContent = value
      tags.appendChild(tag)
    }
    const meta = document.createElement('div')
    meta.className = 'meta'
    const sizeMb = Math.max(0.1, b.fileSize / 1024 / 1024).toFixed(1)
    const lastRead = b.lastReadAt ? `上次阅读 ${new Date(b.lastReadAt).toLocaleDateString('zh-CN')}` : '未读'
    meta.textContent = `${sizeMb} MB · ${lastRead}`
    card.title = [b.title, b.author, series, language, b.description]
      .filter(Boolean)
      .join('\n')
    card.append(cover, t, a)
    if (tags.childElementCount > 0) card.appendChild(tags)
    card.appendChild(meta)
    card.addEventListener('click', () => openLibraryBook(b))
    libraryGrid.appendChild(card)
  }
}

function renderLibraryEmptyState() {
  const empty = document.createElement('div')
  empty.className = 'library-empty'

  const icon = document.createElement('img')
  icon.className = 'library-empty-icon'
  icon.src = '/app-icon.png'
  icon.alt = ''

  const title = document.createElement('div')
  title.className = 'library-empty-title'
  title.textContent = '书架等待第一卷'

  const subtitle = document.createElement('div')
  subtitle.className = 'library-empty-subtitle'
  subtitle.textContent = '还没有本地作品'

  const actions = document.createElement('div')
  actions.className = 'library-empty-actions'

  const importFile = document.createElement('button')
  importFile.className = 'btn btn-primary'
  importFile.textContent = '导入 EPUB'
  importFile.addEventListener('click', () => libraryImportInput.click())

  const importFolder = document.createElement('button')
  importFolder.className = 'btn'
  importFolder.textContent = '导入文件夹'
  importFolder.addEventListener('click', () => libraryFolderInput.click())

  const moreSources = document.createElement('button')
  moreSources.className = 'btn btn-subtle'
  moreSources.textContent = '更多来源'
  moreSources.addEventListener('click', () => {
    librarySourcePanel.open = true
    libraryPathInput.focus()
  })

  actions.append(importFile, importFolder, moreSources)
  empty.append(icon, title, subtitle, actions)
  libraryGrid.replaceChildren(empty)
}

async function openLibraryBook(book: LibraryBook) {
  try {
    await reader.openFromLibraryId(book.id, viewer)
    await bridge.touchLibraryLastRead(book.id).catch(() => {})
    libraryView.hidden = true
    emptyState.hidden = true
    statusbar.hidden = false
    $('#prev-zone').hidden = false
    $('#next-zone').hidden = false
    document.body.classList.add('reading-active')
  } catch (e: any) {
    showError(`打开失败：${e?.message || e}`)
  }
}

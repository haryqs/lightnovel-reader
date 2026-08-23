import { ReaderCore, type PageMode, type ReaderLayoutSettings, type TocItem } from './reader-core'
import {
  bridge,
  hasNativeBridge,
  isBridgeError,
  type AppUpdateInfo,
  type AppUpdateInstallProgress,
  type InstalledPlugin,
  type LibraryBook,
  type LibrarySourceRecord,
  type OpdsEntry,
  type OpdsFeed,
  type OpdsSource,
  type PluginBookDetail,
  type PluginChapterContent,
  type PluginInstallPreview,
  type PluginPackageSignature,
  type PluginRepositoryEntry,
  type PluginSearchPage,
  type PluginSearchResult,
  type PluginSourceDescriptor,
  type RemoteLibrarySource,
  type UserDataBackupInspection,
} from './platform'
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

async function configureServiceWorkerBoundary() {
  if (!('serviceWorker' in navigator)) return

  if (isTauriRuntime()) {
    try {
      const registrations = await navigator.serviceWorker.getRegistrations()
      await Promise.all(registrations.map((registration) => registration.unregister()))
      if ('caches' in window) {
        const cacheNames = await caches.keys()
        await Promise.all(cacheNames.map((cacheName) => caches.delete(cacheName)))
      }
    } catch (error) {
      console.warn('清理桌面端旧 PWA 缓存失败', error)
    }
    return
  }

  if (!import.meta.env.PROD) return

  const register = () => {
    void navigator.serviceWorker.register('/sw.js', { scope: '/' }).catch((error) => {
      console.warn('注册 PWA Service Worker 失败', error)
    })
  }
  if (document.readyState === 'complete') register()
  else window.addEventListener('load', register, { once: true })
}

void configureServiceWorkerBoundary()

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
const libraryFilterSelect = $<HTMLSelectElement>('#library-filter')
const librarySortSelect = $<HTMLSelectElement>('#library-sort')
const libraryResultSummary = $<HTMLElement>('#library-result-summary')
const libraryBackupControls = $<HTMLElement>('#library-backup-controls')
const libraryBackupButton = $<HTMLButtonElement>('#btn-library-backup')
const libraryBackupInspectButton = $<HTMLButtonElement>('#btn-library-backup-inspect')
const libraryBackupStatus = $<HTMLElement>('#library-backup-status')
const backupInspectionDialog = $<HTMLDialogElement>('#backup-inspection-dialog')
const backupInspectionDialogCloseButton = $<HTMLButtonElement>('#btn-backup-inspection-dialog-close')
const backupInspectionCloseButton = $<HTMLButtonElement>('#btn-backup-inspection-close')
const libraryRemoteSourceSelect = $<HTMLSelectElement>('#library-remote-source')
const libraryReadPreferenceSelect = $<HTMLSelectElement>('#library-read-preference')
const librarySourcePanel = $<HTMLDetailsElement>('#library-source-panel')
const appUpdateControls = $<HTMLElement>('#app-update-controls')
const appUpdateStatus = $<HTMLElement>('#app-update-status')
const appUpdateProgress = $<HTMLProgressElement>('#app-update-progress')
const appUpdateBtn = $<HTMLButtonElement>('#btn-app-update')
const appUpdateDialog = $<HTMLDialogElement>('#app-update-dialog')
const appUpdateCurrentVersion = $<HTMLElement>('#app-update-current-version')
const appUpdateTargetVersion = $<HTMLElement>('#app-update-target-version')
const appUpdateReleaseDateRow = $<HTMLElement>('#app-update-release-date-row')
const appUpdateReleaseDate = $<HTMLTimeElement>('#app-update-release-date')
const appUpdateReleaseNotes = $<HTMLElement>('#app-update-release-notes-body')
const appUpdateDialogCloseBtn = $<HTMLButtonElement>('#btn-app-update-dialog-close')
const appUpdateLaterBtn = $<HTMLButtonElement>('#btn-app-update-later')
const appUpdateInstallBtn = $<HTMLButtonElement>('#btn-app-update-install')
// OPDS v0.6
const libraryOpdsPanel = $<HTMLDetailsElement>('#library-opds-panel')
const libraryOpdsUrlHint = $<HTMLElement>('#library-opds-url-hint')
const libraryOpdsUrlText = $<HTMLElement>('#library-opds-url-text')
const libraryOpdsUseUrlBtn = $<HTMLButtonElement>('#btn-library-opds-use-url')
const libraryOpdsDismissUrlBtn = $<HTMLButtonElement>('#btn-library-opds-dismiss-url')
const opdsSourceList = $('#opds-source-list')
const opdsSourceUrlInput = $<HTMLInputElement>('#opds-source-url')
const opdsSourceNameInput = $<HTMLInputElement>('#opds-source-name')
const opdsAddSourceBtn = $<HTMLButtonElement>('#btn-opds-add-source')
const opdsFeedView = $('#opds-feed-view')
const opdsFeedTitle = $('#opds-feed-title')
const opdsFeedGrid = $('#opds-feed-grid')
const opdsFeedBackBtn = $('#btn-opds-feed-back')
const opdsFeedIngestAllBtn = $<HTMLButtonElement>('#btn-opds-feed-ingest-all')
// v0.7 plugin install preview
const libraryPluginPanel = $<HTMLDetailsElement>('#library-plugin-panel')
const pluginSelectPackageBtn = $<HTMLButtonElement>('#btn-plugin-select-package')
const pluginRefreshBtn = $<HTMLButtonElement>('#btn-plugin-refresh')
const pluginRepositoryUrlInput = $<HTMLInputElement>('#plugin-repository-url')
const pluginRepositoryLoadBtn = $<HTMLButtonElement>('#btn-plugin-repository-load')
const pluginRepositoryList = $('#plugin-repository-list')
const pluginInstallPreview = $('#plugin-install-preview')
const pluginInstalledList = $('#plugin-installed-list')
// OPDS session state: track current browsing context
let opdsFeedCache: OpdsFeed | null = null
let opdsSourceCache: OpdsSource | null = null
let pendingPluginPackagePath = ''
let pendingRepositoryPackage: {
  packageUrl: string
  packageSha256: string
  signature?: PluginPackageSignature
} | null = null
let pendingPluginPreview: PluginInstallPreview | null = null
let pluginSources: PluginSourceDescriptor[] = []
let pluginSearchState: {
  source: PluginSourceDescriptor
  query: string
  page: number
  result: PluginSearchPage
} | null = null
let dismissedOpdsUrlHint = ''
let libraryBooks: LibraryBook[] = []
let librarySearchTimer: number | null = null
let appUpdateBusy = false
let libraryBackupBusy = false
type LibraryReadPreference = 'auto' | 'builtin' | 'browser' | 'external'
const LIBRARY_READ_PREFERENCE_KEY = 'reader.libraryReadPreference'
const LIBRARY_READ_PREFERENCES = new Set<LibraryReadPreference>(['auto', 'builtin', 'browser', 'external'])
const REMOTE_SOURCE_LABEL: Record<RemoteLibrarySource, string> = {
  anilist: 'AniList',
  bangumi: 'Bangumi（中文/ACG 元数据）',
  narou: '小説家になろう（Web小说元数据）',
  aozora: '青空文库（公共版权经典）',
}
libraryPathInput.value = localStorage.getItem(LIBRARY_PATH_KEY) || DEFAULT_CALIBRE_LIBRARY

function readLibraryReadPreference(): LibraryReadPreference {
  const value = localStorage.getItem(LIBRARY_READ_PREFERENCE_KEY) as LibraryReadPreference | null
  return value && LIBRARY_READ_PREFERENCES.has(value) ? value : 'auto'
}

function applyLibraryReadPreference(value: LibraryReadPreference) {
  const preference = LIBRARY_READ_PREFERENCES.has(value) ? value : 'auto'
  localStorage.setItem(LIBRARY_READ_PREFERENCE_KEY, preference)
  libraryReadPreferenceSelect.value = preference
}

applyLibraryReadPreference(readLibraryReadPreference())

appUpdateControls.hidden = !isTauriRuntime()
libraryBackupControls.hidden = !isTauriRuntime()

function setLibraryBackupBusy(busy: boolean, action: 'export' | 'inspect') {
  libraryBackupBusy = busy
  libraryBackupButton.disabled = busy
  libraryBackupInspectButton.disabled = busy
  libraryBackupButton.textContent = busy && action === 'export' ? '备份中…' : '备份数据'
  libraryBackupInspectButton.textContent = busy && action === 'inspect' ? '校验中…' : '校验备份'
}

async function exportUserDataBackup() {
  if (libraryBackupBusy) return
  setLibraryBackupBusy(true, 'export')
  libraryBackupControls.dataset.state = 'working'
  libraryBackupStatus.textContent = '请选择保存位置'
  libraryBackupStatus.title = ''
  try {
    const result = await bridge.exportUserDataBackup()
    if (!result) {
      libraryBackupControls.dataset.state = 'idle'
      libraryBackupStatus.textContent = '已取消'
      return
    }
    const summary = `已备份 ${result.fileCount} 个文件 · ${formatBytes(result.totalBytes)}`
    libraryBackupControls.dataset.state = 'success'
    libraryBackupStatus.textContent = summary
    libraryBackupStatus.title = result.path
  } catch (error) {
    const message = `备份失败：${formatError(error)}`
    libraryBackupControls.dataset.state = 'error'
    libraryBackupStatus.textContent = message
    libraryBackupStatus.title = message
  } finally {
    setLibraryBackupBusy(false, 'export')
  }
}

function formatBackupDate(createdAt: number): string {
  const date = new Date(createdAt)
  if (Number.isNaN(date.getTime())) return String(createdAt)
  return new Intl.DateTimeFormat('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  }).format(date)
}

function showBackupInspection(result: UserDataBackupInspection) {
  $('#backup-inspection-version').textContent = `${versionLabel(result.sourceAppVersion)} · schema v${result.schemaVersion}`
  $('#backup-inspection-created-at').textContent = formatBackupDate(result.createdAt)
  $('#backup-inspection-books').textContent = String(result.libraryBookCount)
  $('#backup-inspection-progress').textContent = String(result.readingProgressCount)
  $('#backup-inspection-annotations').textContent = String(result.annotationCount)
  $('#backup-inspection-plugins').textContent = String(result.pluginCount)
  $('#backup-inspection-epubs').textContent = String(result.epubFileCount)
  $('#backup-inspection-payload').textContent = `${result.fileCount} 个文件 · ${formatBytes(result.totalBytes)}`
  $('#backup-inspection-path').textContent = result.path
  const warnings = $('#backup-inspection-warnings')
  warnings.replaceChildren(...result.warnings.map((warning) => {
    const item = document.createElement('li')
    item.textContent = warning
    return item
  }))
  warnings.hidden = result.warnings.length === 0
  backupInspectionDialog.showModal()
}

async function inspectUserDataBackup() {
  if (libraryBackupBusy) return
  setLibraryBackupBusy(true, 'inspect')
  libraryBackupControls.dataset.state = 'working'
  libraryBackupStatus.textContent = '请选择备份目录'
  libraryBackupStatus.title = ''
  try {
    const result = await bridge.inspectUserDataBackup()
    if (!result) {
      libraryBackupControls.dataset.state = 'idle'
      libraryBackupStatus.textContent = '已取消校验'
      return
    }
    libraryBackupControls.dataset.state = 'success'
    libraryBackupStatus.textContent = `校验通过 · ${result.libraryBookCount} 本书 · ${result.annotationCount} 个标注`
    libraryBackupStatus.title = result.path
    showBackupInspection(result)
  } catch (error) {
    const message = `校验失败：${formatError(error)}`
    libraryBackupControls.dataset.state = 'error'
    libraryBackupStatus.textContent = message
    libraryBackupStatus.title = message
  } finally {
    setLibraryBackupBusy(false, 'inspect')
  }
}

function setAppUpdateState(
  state: 'idle' | 'checking' | 'available' | 'installing' | 'success' | 'error',
  status: string,
  buttonLabel: string,
  disabled = false,
) {
  appUpdateControls.dataset.state = state
  appUpdateStatus.textContent = status
  appUpdateStatus.title = status
  appUpdateBtn.textContent = buttonLabel
  appUpdateBtn.disabled = disabled
  if (state !== 'installing') {
    appUpdateProgress.hidden = true
    appUpdateProgress.removeAttribute('value')
    delete appUpdateProgress.dataset.stage
  }
}

function versionLabel(version: string): string {
  return version.startsWith('v') ? version : `v${version}`
}

function formatUpdateDate(value: string): string {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return value
  return new Intl.DateTimeFormat('zh-CN', {
    year: 'numeric',
    month: 'long',
    day: 'numeric',
  }).format(date)
}

function confirmAppUpdate(update: AppUpdateInfo): Promise<boolean> {
  appUpdateCurrentVersion.textContent = versionLabel(update.currentVersion)
  appUpdateTargetVersion.textContent = versionLabel(update.version)

  const date = update.date?.trim()
  appUpdateReleaseDateRow.hidden = !date
  appUpdateReleaseDate.textContent = date ? formatUpdateDate(date) : ''
  appUpdateReleaseDate.dateTime = date || ''

  const notes = update.body?.trim()
  appUpdateReleaseNotes.textContent = notes
    ? notes.slice(0, 4_000)
    : '此版本未提供单独的更新说明。'

  return new Promise((resolve, reject) => {
    const onClose = () => {
      resolve(appUpdateDialog.returnValue === 'install')
    }
    appUpdateDialog.addEventListener('close', onClose, { once: true })
    appUpdateDialog.returnValue = 'later'
    try {
      appUpdateDialog.showModal()
      window.requestAnimationFrame(() => {
        if (appUpdateDialog.open) appUpdateLaterBtn.focus()
      })
    } catch (error) {
      appUpdateDialog.removeEventListener('close', onClose)
      reject(error)
    }
  })
}

function closeAppUpdateDialog(action: 'later' | 'install'): void {
  if (appUpdateDialog.open) appUpdateDialog.close(action)
}

appUpdateDialogCloseBtn.addEventListener('click', () => closeAppUpdateDialog('later'))
appUpdateLaterBtn.addEventListener('click', () => closeAppUpdateDialog('later'))
appUpdateInstallBtn.addEventListener('click', () => closeAppUpdateDialog('install'))
appUpdateDialog.addEventListener('cancel', (event) => {
  event.preventDefault()
  closeAppUpdateDialog('later')
})

function showAppUpdateProgress(progress: AppUpdateInstallProgress, nextVersion: string): void {
  appUpdateProgress.hidden = false
  appUpdateProgress.dataset.stage = progress.stage

  if (progress.stage === 'installing') {
    appUpdateProgress.max = 100
    appUpdateProgress.value = 100
    appUpdateStatus.textContent = `下载完成，正在验签并安装 ${nextVersion}…`
    appUpdateStatus.title = appUpdateStatus.textContent
    return
  }

  const downloadedBytes = Math.max(0, progress.downloadedBytes)
  const totalBytes = progress.totalBytes && progress.totalBytes > 0
    ? progress.totalBytes
    : undefined
  if (!totalBytes) {
    appUpdateProgress.removeAttribute('value')
    appUpdateStatus.textContent = `正在下载 ${nextVersion}… ${formatBytes(downloadedBytes)}`
  } else {
    const percent = Math.min(100, Math.round((downloadedBytes / totalBytes) * 100))
    appUpdateProgress.max = 100
    appUpdateProgress.value = percent
    appUpdateStatus.textContent = `正在下载 ${nextVersion}… ${percent}%（${formatBytes(downloadedBytes)} / ${formatBytes(totalBytes)}）`
  }
  appUpdateStatus.title = appUpdateStatus.textContent
}

async function handleAppUpdate() {
  if (appUpdateBusy) return
  appUpdateBusy = true
  setAppUpdateState('checking', '正在连接更新服务…', '检查中…', true)
  try {
    const update = await bridge.checkAppUpdate()
    if (!update) {
      setAppUpdateState('success', '当前已是最新版本', '再次检查')
      return
    }

    const nextVersion = versionLabel(update.version)
    setAppUpdateState('available', `${nextVersion} 可用`, '安装更新')
    const confirmed = await confirmAppUpdate(update)
    if (!confirmed) return

    setAppUpdateState('installing', `准备下载 ${nextVersion}…`, '安装中…', true)
    appUpdateProgress.hidden = false
    appUpdateProgress.removeAttribute('value')
    appUpdateProgress.dataset.stage = 'downloading'
    await bridge.installAppUpdate((progress) => showAppUpdateProgress(progress, nextVersion))
    setAppUpdateState('success', '更新已安装，正在重启…', '正在重启…', true)
  } catch (error) {
    const message = formatError(error)
    console.error('应用更新失败', error)
    setAppUpdateState('error', message, '重试更新')
  } finally {
    appUpdateBusy = false
  }
}

$('#btn-library')?.addEventListener('click', openLibrary)
$('#btn-library-close')?.addEventListener('click', () => { libraryView.hidden = true })
appUpdateBtn.addEventListener('click', () => void handleAppUpdate())
$('#btn-library-refresh')?.addEventListener('click', refreshLibraryBooks)
libraryBackupButton.addEventListener('click', () => void exportUserDataBackup())
libraryBackupInspectButton.addEventListener('click', () => void inspectUserDataBackup())
backupInspectionDialogCloseButton.addEventListener('click', () => backupInspectionDialog.close())
backupInspectionCloseButton.addEventListener('click', () => backupInspectionDialog.close())
backupInspectionDialog.addEventListener('cancel', (event) => {
  event.preventDefault()
  backupInspectionDialog.close()
})
$('#btn-library-import-epub')?.addEventListener('click', () => libraryImportInput.click())
$('#btn-library-import-folder')?.addEventListener('click', () => libraryFolderInput.click())
$('#btn-library-import-calibre')?.addEventListener('click', importCalibreLibrary)
libraryReadPreferenceSelect.addEventListener('change', () => {
  applyLibraryReadPreference(libraryReadPreferenceSelect.value as LibraryReadPreference)
  if (!libraryView.hidden) renderLibraryBooks()
})
libraryFilterSelect.addEventListener('change', () => renderLibraryBooks())
librarySortSelect.addEventListener('change', () => renderLibraryBooks())
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
  if (updateOpdsUrlHint()) {
    if (librarySearchTimer !== null) {
      window.clearTimeout(librarySearchTimer)
      librarySearchTimer = null
    }
    return
  }
  if (librarySearchTimer !== null) window.clearTimeout(librarySearchTimer)
  librarySearchTimer = window.setTimeout(() => {
    librarySearchTimer = null
    refreshLibraryBooks()
  }, 180)
})
libraryOpdsUseUrlBtn.addEventListener('click', () => useDetectedOpdsUrl())
libraryOpdsDismissUrlBtn.addEventListener('click', () => dismissDetectedOpdsUrl())
libraryPluginPanel.addEventListener('toggle', () => {
  if (libraryPluginPanel.open) void refreshInstalledPlugins()
})
pluginSelectPackageBtn.addEventListener('click', () => void selectPluginPackage())
pluginRefreshBtn.addEventListener('click', () => void refreshInstalledPlugins())
pluginRepositoryLoadBtn.addEventListener('click', () => void loadPluginRepository())
pluginRepositoryUrlInput.addEventListener('keydown', (event) => {
  if (event.key === 'Enter') void loadPluginRepository()
})

function detectOpdsFeedUrl(value: string): string | null {
  const raw = value.trim()
  if (!raw) return null
  let url: URL
  try {
    url = new URL(raw)
  } catch {
    return null
  }
  if (url.protocol !== 'http:' && url.protocol !== 'https:') return null
  const haystack = `${url.hostname}${url.pathname}${url.search}`.toLowerCase()
  if (
    haystack.includes('opds') ||
    haystack.includes('feed') ||
    haystack.includes('catalog') ||
    /\.(atom|xml|json)$/i.test(url.pathname)
  ) {
    return url.href
  }
  return null
}

function suggestOpdsSourceName(url: string): string {
  try {
    const parsed = new URL(url)
    return parsed.hostname.replace(/^www\./i, '')
  } catch {
    return url
  }
}

function updateOpdsUrlHint(): boolean {
  const detectedUrl = detectOpdsFeedUrl(librarySearchInput.value)
  if (!detectedUrl || detectedUrl === dismissedOpdsUrlHint) {
    libraryOpdsUrlHint.hidden = true
    libraryOpdsUrlText.textContent = ''
    return false
  }
  libraryOpdsUrlText.textContent = detectedUrl
  libraryOpdsUrlHint.hidden = false
  return true
}

function useDetectedOpdsUrl() {
  const detectedUrl = detectOpdsFeedUrl(librarySearchInput.value)
  if (!detectedUrl) {
    libraryOpdsUrlHint.hidden = true
    return
  }
  libraryOpdsPanel.open = true
  opdsSourceUrlInput.value = detectedUrl
  if (!opdsSourceNameInput.value.trim()) {
    opdsSourceNameInput.value = suggestOpdsSourceName(detectedUrl)
  }
  dismissedOpdsUrlHint = detectedUrl
  libraryOpdsUrlHint.hidden = true
  librarySearchInput.value = ''
  void refreshOpdsSources()
  void refreshLibraryBooks()
  opdsSourceNameInput.focus()
}

function dismissDetectedOpdsUrl() {
  dismissedOpdsUrlHint = detectOpdsFeedUrl(librarySearchInput.value) || ''
  libraryOpdsUrlHint.hidden = true
}

async function selectPluginPackage() {
  if (!isTauriRuntime()) {
    showPluginPanelMessage('源插件安装需要 Tauri 桌面窗口。', true)
    return
  }
  pluginSelectPackageBtn.disabled = true
  pluginSelectPackageBtn.textContent = '选择中…'
  try {
    const path = await bridge.selectPluginPackagePath()
    if (!path) return
    pendingPluginPackagePath = path
    pendingRepositoryPackage = null
    pendingPluginPreview = await bridge.inspectPluginPackage(path)
    renderPluginInstallPreview(pendingPluginPreview, path)
  } catch (e: any) {
    showPluginPanelMessage(`读取插件安装包失败：${formatError(e)}`, true)
  } finally {
    pluginSelectPackageBtn.disabled = false
    pluginSelectPackageBtn.textContent = '选择插件 zip'
  }
}

async function loadPluginRepository() {
  if (!isTauriRuntime()) {
    renderPluginRepositoryMessage('官方索引需要 Tauri 桌面窗口。', true)
    return
  }
  const url = pluginRepositoryUrlInput.value.trim()
  if (!url) {
    renderPluginRepositoryMessage('请输入官方插件索引 JSON URL。', true)
    return
  }
  clearPluginPreview()
  pluginRepositoryLoadBtn.disabled = true
  pluginRepositoryLoadBtn.textContent = '加载中…'
  pluginRepositoryList.innerHTML = '<div class="plugin-empty">读取官方索引…</div>'
  try {
    const catalog = await bridge.loadPluginRepositoryIndex(url)
    renderPluginRepository(catalog.index.entries, catalog.validation.warnings)
  } catch (e: any) {
    renderPluginRepositoryMessage(`加载官方索引失败：${formatError(e)}`, true)
  } finally {
    pluginRepositoryLoadBtn.disabled = false
    pluginRepositoryLoadBtn.textContent = '加载官方索引'
  }
}

function renderPluginRepository(entries: PluginRepositoryEntry[], warnings: string[]) {
  pluginRepositoryList.innerHTML = ''
  if (warnings.length > 0) {
    const warning = document.createElement('div')
    warning.className = 'plugin-message'
    warning.textContent = warnings.join(' · ')
    pluginRepositoryList.appendChild(warning)
  }
  if (entries.length === 0) {
    const empty = document.createElement('div')
    empty.className = 'plugin-empty'
    empty.textContent = '官方索引暂无插件。'
    pluginRepositoryList.appendChild(empty)
    return
  }
  for (const entry of entries) {
    const row = document.createElement('div')
    row.className = 'plugin-repository-row'

    const main = document.createElement('div')
    main.className = 'plugin-installed-main'
    const name = document.createElement('div')
    name.className = 'plugin-installed-name'
    name.textContent = `${entry.manifest.name} ${entry.manifest.version}`
    const meta = document.createElement('div')
    meta.className = 'plugin-installed-meta'
    meta.textContent = [
      entry.manifest.id,
      pluginLegalLabel(entry.manifest.legal.kind),
      entry.manifest.capabilities.map(pluginCapabilityLabel).join(', ') || '基础搜索',
      entry.packageSize ? formatBytes(entry.packageSize) : '未知大小',
      entry.signature ? `待下载验签 · ${entry.signature.keyId}` : '未签名 · 人工白名单',
    ].join(' · ')
    main.append(name, meta)

    const actions = document.createElement('div')
    actions.className = 'plugin-installed-actions'
    const inspect = document.createElement('button')
    inspect.className = 'btn btn-primary'
    inspect.textContent = '校验包'
    inspect.addEventListener('click', () => {
      void inspectRepositoryEntry(entry, inspect)
    })
    actions.appendChild(inspect)
    if (entry.sourceUrl) {
      const source = document.createElement('button')
      source.className = 'btn'
      source.textContent = '源码'
      source.addEventListener('click', () => {
        void openPluginRepositorySource(entry.sourceUrl as string)
      })
      actions.appendChild(source)
    }

    row.append(main, actions)
    pluginRepositoryList.appendChild(row)
  }
}

function renderPluginRepositoryMessage(message: string, error = false) {
  pluginRepositoryList.innerHTML = ''
  const item = document.createElement('div')
  item.className = error ? 'plugin-message plugin-message-error' : 'plugin-message'
  item.textContent = message
  pluginRepositoryList.appendChild(item)
}

async function openPluginRepositorySource(url: string) {
  try {
    await bridge.openExternal(url)
  } catch (e: any) {
    showPluginPanelMessage(`打开源码地址失败：${formatError(e)}`, true)
  }
}

async function inspectRepositoryEntry(entry: PluginRepositoryEntry, button: HTMLButtonElement) {
  button.disabled = true
  button.textContent = '校验中…'
  try {
    const preview = await bridge.inspectRepositoryPluginPackage(
      entry.packageUrl,
      entry.packageSha256,
      entry.signature,
    )
    pendingPluginPackagePath = ''
    pendingRepositoryPackage = {
      packageUrl: entry.packageUrl,
      packageSha256: entry.packageSha256,
      signature: entry.signature,
    }
    pendingPluginPreview = preview
    renderPluginInstallPreview(preview, entry.packageUrl)
  } catch (e: any) {
    showPluginPanelMessage(`校验官方插件包失败：${formatError(e)}`, true)
  } finally {
    button.disabled = false
    button.textContent = '校验包'
  }
}

function renderPluginInstallPreview(preview: PluginInstallPreview, path: string) {
  pluginInstallPreview.hidden = false
  pluginInstallPreview.innerHTML = ''

  const header = document.createElement('div')
  header.className = 'plugin-preview-header'
  const title = document.createElement('div')
  title.className = 'plugin-preview-title'
  title.textContent = `${preview.manifest.name} ${preview.manifest.version}`
  const id = document.createElement('div')
  id.className = 'plugin-preview-id'
  id.textContent = `${preview.manifest.id} · API ${preview.manifest.apiVersion} · ${formatBytes(preview.entrySize)}`
  header.append(title, id)

  const pathEl = document.createElement('div')
  pathEl.className = 'plugin-preview-path'
  pathEl.textContent = path

  const desc = document.createElement('div')
  desc.className = 'plugin-preview-desc'
  desc.textContent = preview.manifest.description || '该插件未提供简介。'

  const facts = document.createElement('div')
  facts.className = 'plugin-preview-facts'
  facts.append(
    pluginFact('授权', pluginLegalLabel(preview.manifest.legal.kind)),
    pluginFact('权限', preview.manifest.permissions.join(', ') || '无'),
    pluginFact('能力', preview.manifest.capabilities.map(pluginCapabilityLabel).join(', ') || '基础搜索'),
    pluginFact('域名', preview.manifest.domains.join(', ')),
  )
  if (preview.manifest.legal.termsUrl) {
    facts.append(pluginFact('源站条款', preview.manifest.legal.termsUrl))
  }

  const warnings = document.createElement('div')
  warnings.className = preview.validation.warnings.length > 0
    ? 'plugin-preview-warnings'
    : 'plugin-preview-warnings plugin-preview-warnings-ok'
  warnings.textContent = preview.validation.warnings.length > 0
    ? preview.validation.warnings.join(' · ')
    : '未发现额外合规警告。安装动作不会执行代码；只有启用后由用户主动使用来源时才会运行。'

  const confirmation = document.createElement('label')
  confirmation.className = 'plugin-confirm'
  const checkbox = document.createElement('input')
  checkbox.type = 'checkbox'
  const requiresConfirmation = preview.validation.requiresUserLegalConfirmation
    || preview.validation.requiresSourceTermsConfirmation
  checkbox.checked = !requiresConfirmation
  checkbox.disabled = !requiresConfirmation
  const confirmText = document.createElement('span')
  confirmText.textContent = preview.validation.requiresSourceTermsConfirmation
    ? '我已阅读源站条款，并同意按宿主每域限速使用该 official-free 来源'
    : preview.validation.requiresUserLegalConfirmation
      ? '我确认该 user-declared 插件来源与合法性由我自行负责'
      : '官方可收录类型：无需额外来源条款确认'
  confirmation.append(checkbox, confirmText)

  const actions = document.createElement('div')
  actions.className = 'plugin-preview-actions'
  if (preview.manifest.legal.termsUrl) {
    const terms = document.createElement('button')
    terms.className = 'btn'
    terms.textContent = '查看源站条款'
    terms.addEventListener('click', () => {
      void bridge.openExternal(preview.manifest.legal.termsUrl as string)
        .catch((error) => showPluginPanelMessage(`打开源站条款失败：${formatError(error)}`, true))
    })
    actions.appendChild(terms)
  }
  const install = document.createElement('button')
  install.className = 'btn btn-primary'
  install.textContent = '确认安装'
  install.addEventListener('click', () => {
    void installPendingPlugin(checkbox.checked)
  })
  const clear = document.createElement('button')
  clear.className = 'btn'
  clear.textContent = '取消'
  clear.addEventListener('click', clearPluginPreview)
  actions.append(install, clear)

  pluginInstallPreview.append(header, pathEl, desc, facts, warnings, confirmation, actions)
}

function pluginFact(label: string, value: string): HTMLElement {
  const item = document.createElement('div')
  item.className = 'plugin-preview-fact'
  const key = document.createElement('span')
  key.textContent = label
  const val = document.createElement('strong')
  val.textContent = value
  item.append(key, val)
  return item
}

async function installPendingPlugin(confirmUserLegal: boolean) {
  if ((!pendingPluginPackagePath && !pendingRepositoryPackage) || !pendingPluginPreview) return
  const installBtn = pluginInstallPreview.querySelector<HTMLButtonElement>('.plugin-preview-actions .btn-primary')
  if (installBtn) {
    installBtn.disabled = true
    installBtn.textContent = '安装中…'
  }
  try {
    const installed = pendingRepositoryPackage
      ? await bridge.installRepositoryPluginPackage(
        pendingRepositoryPackage.packageUrl,
        pendingRepositoryPackage.packageSha256,
        pendingRepositoryPackage.signature,
      )
      : await bridge.installPluginPackage(pendingPluginPackagePath, confirmUserLegal)
    clearPluginPreview()
    prependPluginSummary(`已安装源插件：${installed.manifest.name} ${installed.manifest.version}`)
    await refreshInstalledPlugins()
  } catch (e: any) {
    showPluginPanelMessage(`安装插件失败：${formatError(e)}`, true)
  } finally {
    if (installBtn) {
      installBtn.disabled = false
      installBtn.textContent = '确认安装'
    }
  }
}

function clearPluginPreview() {
  pendingPluginPackagePath = ''
  pendingRepositoryPackage = null
  pendingPluginPreview = null
  pluginInstallPreview.hidden = true
  pluginInstallPreview.innerHTML = ''
}

async function refreshInstalledPlugins() {
  if (!isTauriRuntime()) {
    pluginInstalledList.innerHTML = '<div class="plugin-empty">源插件需要 Tauri 桌面窗口。</div>'
    return
  }
  pluginInstalledList.innerHTML = '<div class="plugin-empty">读取已安装插件…</div>'
  try {
    const plugins = await bridge.listInstalledPlugins()
    renderInstalledPlugins(plugins)
  } catch (e: any) {
    pluginInstalledList.innerHTML = ''
    showPluginPanelMessage(`读取已安装插件失败：${formatError(e)}`, true)
  }
  await refreshPluginSources()
}

async function refreshPluginSources() {
  const selected = libraryRemoteSourceSelect.value
  for (const option of Array.from(libraryRemoteSourceSelect.options)) {
    if (option.dataset.pluginSource === 'true') option.remove()
  }
  pluginSources = []
  if (!isTauriRuntime()) return
  try {
    pluginSources = await bridge.listPluginSources()
    for (const source of pluginSources) {
      const option = document.createElement('option')
      option.value = `plugin:${source.id}`
      option.dataset.pluginSource = 'true'
      option.textContent = `${source.name}（插件来源）`
      libraryRemoteSourceSelect.appendChild(option)
    }
    if (Array.from(libraryRemoteSourceSelect.options).some((option) => option.value === selected)) {
      libraryRemoteSourceSelect.value = selected
    }
  } catch (e) {
    console.warn('读取正式插件来源失败', e)
  }
}

function renderInstalledPlugins(plugins: InstalledPlugin[]) {
  pluginInstalledList.innerHTML = ''
  if (plugins.length === 0) {
    pluginInstalledList.innerHTML = '<div class="plugin-empty">暂无已安装源插件。</div>'
    return
  }
  for (const plugin of plugins) {
    const row = document.createElement('div')
    row.className = 'plugin-installed-row'
    const main = document.createElement('div')
    main.className = 'plugin-installed-main'
    const name = document.createElement('div')
    name.className = 'plugin-installed-name'
    name.textContent = `${plugin.manifest.name} ${plugin.manifest.version}`
    const meta = document.createElement('div')
    meta.className = 'plugin-installed-meta'
    meta.textContent = [
      plugin.manifest.id,
      pluginLegalLabel(plugin.manifest.legal.kind),
      plugin.enabled ? '已启用' : '已停用',
      plugin.manifest.capabilities.map(pluginCapabilityLabel).join(', ') || '基础搜索',
      `安装于 ${new Date(plugin.installedAt).toLocaleString('zh-CN')}`,
    ].join(' · ')
    main.append(name, meta)

    const side = document.createElement('div')
    side.className = 'plugin-installed-actions'
    const badge = document.createElement('span')
    badge.className = plugin.validation.requiresUserLegalConfirmation
      ? 'plugin-badge plugin-badge-user'
      : 'plugin-badge'
    badge.textContent = plugin.validation.requiresUserLegalConfirmation ? '用户自装' : '可白名单'
    const toggle = document.createElement('button')
    toggle.className = plugin.enabled ? 'btn' : 'btn btn-primary'
    toggle.textContent = plugin.enabled ? '停用' : '启用'
    toggle.title = plugin.enabled ? '停用后运行时不会加载该插件' : '重新允许运行时加载该插件'
    toggle.addEventListener('click', () => {
      void setInstalledPluginEnabled(plugin.manifest.id, !plugin.enabled)
    })
    const uninstall = document.createElement('button')
    uninstall.className = 'btn btn-danger'
    uninstall.textContent = '卸载'
    uninstall.title = '删除本地插件文件'
    uninstall.addEventListener('click', () => {
      void uninstallInstalledPlugin(plugin)
    })
    const testBtn = document.createElement('button')
    testBtn.className = 'btn btn-small'
    testBtn.textContent = '测试'
    testBtn.title = '用测试查询验证 search → getBook → getChapter 完整流程'
    testBtn.addEventListener('click', () => {
      void testPluginRun(plugin.manifest.id)
    })
    side.append(badge, toggle, uninstall, testBtn)
    row.append(main, side)
    pluginInstalledList.appendChild(row)
  }
}

async function setInstalledPluginEnabled(pluginId: string, enabled: boolean) {
  try {
    await bridge.setPluginEnabled(pluginId, enabled)
    await refreshInstalledPlugins()
  } catch (e: any) {
    showPluginPanelMessage(`更新插件状态失败：${formatError(e)}`, true)
  }
}

async function uninstallInstalledPlugin(plugin: InstalledPlugin) {
  const label = `${plugin.manifest.name} ${plugin.manifest.version}`
  if (!window.confirm(`确定卸载源插件「${label}」？\n\n这会删除本地插件文件，但不会影响书库数据。`)) {
    return
  }
  try {
    await bridge.uninstallPlugin(plugin.manifest.id)
    prependPluginSummary(`已卸载源插件：${label}`)
    await refreshInstalledPlugins()
  } catch (e: any) {
    showPluginPanelMessage(`卸载失败：${formatError(e)}`, true)
  }
}

async function testPluginRun(pluginId: string) {
  showPluginPanelMessage(`正在验证插件 ${pluginId} 的 search → getBook → getChapter...`, false)
  try {
    const result = await bridge.testPluginFlow(pluginId, 'test')
    showPluginPanelMessage(
      `插件完整流程通过：搜索 ${result.search.results.length} 条 → ${result.book.title} → ${result.chapter.title}\n${JSON.stringify(result, null, 2)}`,
      false,
    )
  } catch (e: any) {
    showPluginPanelMessage(`测试运行失败：${formatError(e)}`, true)
  }
}

function showPluginPanelMessage(message: string, error = false) {
  pluginInstallPreview.hidden = false
  pluginInstallPreview.innerHTML = ''
  const item = document.createElement('div')
  item.className = error ? 'plugin-message plugin-message-error' : 'plugin-message'
  item.textContent = message
  pluginInstallPreview.appendChild(item)
}

function prependPluginSummary(message: string) {
  const msg = document.createElement('div')
  msg.className = 'library-import-summary'
  msg.textContent = message
  libraryGrid.prepend(msg)
}

function pluginLegalLabel(kind: string): string {
  switch (kind) {
    case 'public-domain':
      return '公共版权'
    case 'open-license':
      return '开放授权'
    case 'official-free':
      return '官方免费'
    case 'user-declared':
      return '用户声明'
    default:
      return kind || '未知'
  }
}

function pluginCapabilityLabel(capability: string): string {
  switch (capability) {
    case 'browse':
      return '浏览目录'
    case 'resolveUrl':
      return '识别链接'
    case 'fetchMetadata':
      return '补全元数据'
    case 'acquire':
      return '请求获取'
    default:
      return capability
  }
}

function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return '0 B'
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 102.4) / 10} KB`
  return `${Math.round(bytes / (1024 * 102.4)) / 10} MB`
}

const librarySearchRemoteBtn = $<HTMLButtonElement>('#btn-library-search-remote')
const libraryBatchLinkBtn = $<HTMLButtonElement>('#btn-library-batch-link')
librarySearchRemoteBtn.addEventListener('click', () => searchRemoteBooks())
libraryBatchLinkBtn.addEventListener('click', () => {
  void showBatchLinkQueue()
})

// 在线找书：用 AniList 拉元数据 → 落库为远程条目（availability=remote）→ 直接展示结果。
// 只取索引/封面/简介，正文一律跳官方外链（版权红线）。
async function searchRemoteBooks() {
  // 输入框同时用于本地筛选；用户输入后立即点击在线搜索时，必须取消尚未触发的
  // 本地防抖刷新，避免它在远程结果返回后覆盖插件/在线来源的结果网格。
  if (librarySearchTimer !== null) {
    window.clearTimeout(librarySearchTimer)
    librarySearchTimer = null
  }
  const query = librarySearchInput.value.trim()
  const selectedSource = libraryRemoteSourceSelect.value || 'anilist'
  const pluginSource = selectedSource.startsWith('plugin:')
    ? pluginSources.find((source) => source.id === selectedSource.slice('plugin:'.length))
    : undefined
  const source = selectedSource as RemoteLibrarySource
  const sourceLabel = pluginSource?.name || REMOTE_SOURCE_LABEL[source] || selectedSource
  if (!query) {
    showError('先在搜索框输入要在线查找的关键词')
    return
  }
  if (selectedSource.startsWith('plugin:') && !pluginSource) {
    showError('该插件来源已停用或不可用，请刷新来源列表')
    return
  }
  if (!isTauriRuntime()) {
    setLibraryOrganizeSummary('在线来源不可用', false)
    libraryGrid.innerHTML = '<div class="library-state">在线找书需要 Tauri 桌面窗口。<br>请在终端运行：<code>npm run tauri dev</code></div>'
    return
  }
  libraryFilterSelect.value = 'all'
  const original = librarySearchRemoteBtn.textContent
  librarySearchRemoteBtn.disabled = true
  librarySearchRemoteBtn.textContent = '搜索中…'
  if (pluginSource) {
    showPluginSourceGridState(`正在 ${sourceLabel} 搜索元数据…`)
  } else {
    libraryGrid.innerHTML = `<div class="library-state">正在 ${sourceLabel} 搜索元数据…</div>`
  }
  try {
    if (pluginSource) {
      await loadPluginSourcePage(pluginSource, query, 1)
    } else {
      libraryBooks = await bridge.searchRemoteLibraryBooksFromSource(source, query)
      renderLibraryBooks()
    }
  } catch (e: any) {
    showPluginSourceGridState(`在线搜索失败：${formatError(e)}`, true)
  } finally {
    librarySearchRemoteBtn.disabled = false
    librarySearchRemoteBtn.textContent = original
  }
}

function canAcquirePluginSource(source: PluginSourceDescriptor): boolean {
  return source.capabilities.includes('acquire')
    && (source.legal.kind === 'public-domain' || source.legal.kind === 'open-license')
}

function showPluginSourceGridState(message: string, error = false) {
  setLibraryOrganizeSummary(error ? '在线来源失败' : '在线来源', false)
  libraryGrid.innerHTML = ''
  const state = document.createElement('div')
  state.className = error ? 'library-state library-state-error' : 'library-state'
  state.textContent = message
  libraryGrid.appendChild(state)
}

async function loadPluginSourcePage(
  source: PluginSourceDescriptor,
  query: string,
  page: number,
) {
  showPluginSourceGridState(`正在 ${source.name} 搜索第 ${page} 页…`)
  const result = await bridge.searchPluginSource(source.id, query, page)
  pluginSearchState = { source, query, page, result }
  renderPluginSourceResults(source, query, page, result)
}

function renderPluginSourceResults(
  source: PluginSourceDescriptor,
  query: string,
  page: number,
  result: PluginSearchPage,
) {
  setLibraryOrganizeSummary(`${result.results.length} 条在线结果`, false)
  libraryGrid.innerHTML = ''
  updateBatchLinkButton([])

  const toolbar = document.createElement('div')
  toolbar.className = 'plugin-source-results-toolbar'
  const summary = document.createElement('div')
  summary.className = 'plugin-source-results-summary'
  summary.textContent = `${source.name} · “${query}” · 第 ${page} 页 · ${result.results.length} 条`
  const paging = document.createElement('div')
  paging.className = 'plugin-source-results-actions'
  if (page > 1) {
    const previous = document.createElement('button')
    previous.className = 'btn btn-subtle'
    previous.textContent = '← 上一页'
    previous.addEventListener('click', () => {
      void loadPluginSourcePage(source, query, page - 1).catch((error) => {
        showPluginSourceGridState(`插件来源搜索失败：${formatError(error)}`, true)
      })
    })
    paging.appendChild(previous)
  }
  if (result.hasMore) {
    const next = document.createElement('button')
    next.className = 'btn btn-primary'
    next.textContent = '下一页 →'
    next.addEventListener('click', () => {
      void loadPluginSourcePage(source, query, page + 1).catch((error) => {
        showPluginSourceGridState(`插件来源搜索失败：${formatError(error)}`, true)
      })
    })
    paging.appendChild(next)
  }
  toolbar.append(summary, paging)
  libraryGrid.appendChild(toolbar)

  if (result.results.length === 0) {
    const empty = document.createElement('div')
    empty.className = 'library-state'
    empty.textContent = '该插件来源没有返回匹配结果。'
    libraryGrid.appendChild(empty)
    return
  }

  for (const item of result.results) {
    const card = document.createElement('article')
    card.className = 'book-card book-card-remote plugin-source-card'
    const cover = document.createElement('div')
    cover.className = 'cover'
    if (item.coverUrl) {
      const image = document.createElement('img')
      image.loading = 'lazy'
      image.decoding = 'async'
      image.src = item.coverUrl
      image.alt = ''
      cover.appendChild(image)
    } else {
      cover.textContent = '来源'
    }
    const title = document.createElement('div')
    title.className = 'title'
    title.textContent = item.title
    const author = document.createElement('div')
    author.className = 'author'
    author.textContent = item.author || '作者未提供'
    const tags = document.createElement('div')
    tags.className = 'book-tags'
    for (const text of [pluginLegalLabel(source.legal.kind), source.language?.toUpperCase()].filter(Boolean)) {
      const tag = document.createElement('span')
      tag.textContent = text as string
      tags.appendChild(tag)
    }
    const description = document.createElement('div')
    description.className = 'plugin-source-card-summary'
    description.textContent = item.summary || source.description || '该结果未提供简介。'
    const actions = document.createElement('div')
    actions.className = 'book-card-actions'
    const detail = document.createElement('button')
    detail.className = 'btn btn-primary'
    detail.textContent = '查看章节'
    detail.addEventListener('click', () => {
      void showPluginSourceBook(source, item)
    })
    const collect = document.createElement('button')
    collect.className = 'btn btn-subtle'
    collect.textContent = '收藏来源'
    collect.addEventListener('click', () => {
      void collectPluginSourceBook(source, item.url, collect)
    })
    actions.appendChild(detail)
    if (canAcquirePluginSource(source)) {
      const acquire = document.createElement('button')
      acquire.className = 'btn btn-subtle'
      acquire.textContent = '获取并阅读'
      acquire.addEventListener('click', () => {
        void acquirePluginSourceBook(source, item.url, acquire)
      })
      actions.appendChild(acquire)
    }
    const external = document.createElement('button')
    external.className = 'btn btn-subtle'
    external.textContent = '打开源站'
    external.addEventListener('click', () => {
      void bridge.openExternal(item.url).catch((error) => showError(formatError(error)))
    })
    actions.append(collect, external)
    card.append(cover, title, author, tags, description, actions)
    libraryGrid.appendChild(card)
  }
}

async function showPluginSourceBook(source: PluginSourceDescriptor, item: PluginSearchResult) {
  showPluginSourceGridState(`正在读取《${item.title}》详情…`)
  try {
    const book = await bridge.getPluginSourceBook(source.id, item.url)
    renderPluginSourceBook(source, book)
  } catch (error) {
    showPluginSourceGridState(`读取插件书籍详情失败：${formatError(error)}`, true)
  }
}

function renderPluginSourceBook(source: PluginSourceDescriptor, book: PluginBookDetail) {
  setLibraryOrganizeSummary('来源详情', false)
  libraryGrid.innerHTML = ''
  updateBatchLinkButton([])
  const panel = document.createElement('section')
  panel.className = 'plugin-source-book'
  const toolbar = document.createElement('div')
  toolbar.className = 'plugin-source-book-toolbar'
  const back = document.createElement('button')
  back.className = 'btn btn-subtle'
  back.textContent = '← 返回搜索结果'
  back.addEventListener('click', () => {
    if (pluginSearchState) {
      renderPluginSourceResults(
        pluginSearchState.source,
        pluginSearchState.query,
        pluginSearchState.page,
        pluginSearchState.result,
      )
    } else {
      void refreshLibraryBooks()
    }
  })
  const toolbarActions = document.createElement('div')
  toolbarActions.className = 'plugin-source-results-actions'
  const collect = document.createElement('button')
  collect.className = 'btn btn-primary'
  collect.textContent = '收藏来源'
  collect.addEventListener('click', () => {
    void collectPluginSourceBook(source, book.url, collect)
  })
  if (canAcquirePluginSource(source)) {
    const acquire = document.createElement('button')
    acquire.className = 'btn btn-primary'
    acquire.textContent = '获取并阅读'
    acquire.addEventListener('click', () => {
      void acquirePluginSourceBook(source, book.url, acquire)
    })
    toolbarActions.appendChild(acquire)
  }
  const external = document.createElement('button')
  external.className = 'btn btn-subtle'
  external.textContent = '打开源站'
  external.addEventListener('click', () => {
    void bridge.openExternal(book.url).catch((error) => showError(formatError(error)))
  })
  toolbarActions.append(collect, external)
  toolbar.append(back, toolbarActions)

  const header = document.createElement('div')
  header.className = 'plugin-source-book-header'
  if (book.coverUrl) {
    const cover = document.createElement('img')
    cover.className = 'plugin-source-book-cover'
    cover.src = book.coverUrl
    cover.alt = ''
    header.appendChild(cover)
  }
  const copy = document.createElement('div')
  const title = document.createElement('h3')
  title.textContent = book.title
  const meta = document.createElement('div')
  meta.className = 'plugin-source-book-meta'
  meta.textContent = [
    book.author || '作者未提供',
    source.name,
    pluginLegalLabel(source.legal.kind),
    `${book.chapters.length} 章`,
  ].join(' · ')
  const description = document.createElement('p')
  description.textContent = book.description || '该来源未提供书籍简介。'
  copy.append(title, meta, description)
  header.appendChild(copy)

  const chapters = document.createElement('div')
  chapters.className = 'plugin-source-chapters'
  const visibleChapters = book.chapters.slice(0, 200)
  for (const chapter of visibleChapters) {
    const row = document.createElement('div')
    row.className = 'plugin-source-chapter-row'
    const label = document.createElement('div')
    label.className = 'plugin-source-chapter-title'
    label.textContent = chapter.group ? `${chapter.group} · ${chapter.title}` : chapter.title
    const preview = document.createElement('button')
    preview.className = 'btn btn-small'
    preview.textContent = '预览正文'
    preview.addEventListener('click', () => {
      void previewPluginSourceChapter(source, book, chapter, preview)
    })
    row.append(label, preview)
    chapters.appendChild(row)
  }
  if (book.chapters.length > visibleChapters.length) {
    const omitted = document.createElement('div')
    omitted.className = 'plugin-source-chapters-omitted'
    omitted.textContent = `章节较多，当前先展示前 ${visibleChapters.length} 章。`
    chapters.appendChild(omitted)
  }
  if (book.chapters.length === 0) {
    const empty = document.createElement('div')
    empty.className = 'plugin-empty'
    empty.textContent = '该来源没有返回章节。'
    chapters.appendChild(empty)
  }
  panel.append(toolbar, header, chapters)
  libraryGrid.appendChild(panel)
}

async function previewPluginSourceChapter(
  source: PluginSourceDescriptor,
  book: PluginBookDetail,
  chapter: PluginBookDetail['chapters'][number],
  button: HTMLButtonElement,
) {
  button.disabled = true
  button.textContent = '读取中…'
  try {
    const content = await bridge.getPluginSourceChapter(source.id, chapter.url)
    renderPluginSourceChapter(source, book, content)
  } catch (error) {
    showError(`读取插件章节失败：${formatError(error)}`)
  } finally {
    button.disabled = false
    button.textContent = '预览正文'
  }
}

function renderPluginSourceChapter(
  source: PluginSourceDescriptor,
  book: PluginBookDetail,
  content: PluginChapterContent,
) {
  libraryGrid.innerHTML = ''
  const panel = document.createElement('section')
  panel.className = 'plugin-source-book plugin-source-chapter-preview'
  const back = document.createElement('button')
  back.className = 'btn btn-subtle'
  back.textContent = '← 返回章节列表'
  back.addEventListener('click', () => renderPluginSourceBook(source, book))
  const title = document.createElement('h3')
  title.textContent = content.title
  const note = document.createElement('div')
  note.className = 'plugin-source-book-meta'
  note.textContent = '正文已由 reading-core 清洗；此处以纯文本预览，不会加载插件返回的远程资源。'
  const parsed = new DOMParser().parseFromString(content.html, 'text/html')
  const fullText = parsed.body.textContent?.replace(/\s+/g, ' ').trim() || ''
  const text = document.createElement('pre')
  text.className = 'plugin-source-chapter-text'
  text.textContent = fullText.length > 50_000
    ? `${fullText.slice(0, 50_000)}\n\n……预览已截断……`
    : fullText
  panel.append(back, title, note, text)
  libraryGrid.appendChild(panel)
}

async function collectPluginSourceBook(
  source: PluginSourceDescriptor,
  bookUrl: string,
  button: HTMLButtonElement,
) {
  button.disabled = true
  const original = button.textContent
  button.textContent = '收藏中…'
  try {
    const collected = await bridge.collectPluginSourceBook(source.id, bookUrl)
    librarySearchInput.value = ''
    pluginSearchState = null
    await refreshLibraryBooks()
    prependPluginSummary(`已收藏插件来源：${collected.title} · ${source.name}`)
  } catch (error) {
    showError(`收藏插件来源失败：${formatError(error)}`)
  } finally {
    button.disabled = false
    button.textContent = original
  }
}

async function acquirePluginSourceBook(
  source: PluginSourceDescriptor,
  bookUrl: string,
  button: HTMLButtonElement,
) {
  button.disabled = true
  const original = button.textContent
  button.textContent = '获取中…'
  try {
    const acquired = await bridge.acquirePluginSourceBook(source.id, bookUrl)
    librarySearchInput.value = ''
    pluginSearchState = null
    await refreshLibraryBooks()
    prependPluginSummary(`已获取开放资源：${acquired.title} · ${source.name}`)
    await openAcquiredLibraryBook(acquired)
  } catch (error) {
    showError(`获取插件 EPUB 失败：${formatError(error)}`)
  } finally {
    button.disabled = false
    button.textContent = original
  }
}

async function openLibrary() {
  libraryView.hidden = false
  await Promise.all([refreshPluginSources(), refreshLibraryBooks()])
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
    libraryGrid.innerHTML = `<div class="library-state library-state-error">读取书库失败：${formatError(e)}</div>`
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
    libraryGrid.innerHTML = `<div class="library-state library-state-error">导入失败：${formatError(e)}</div>`
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
  if (isBridgeError(err)) {
    return err.details
      ? `${err.message}（${err.code}: ${err.details}）`
      : `${err.message}（${err.code}）`
  }
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

function remoteStatusLabel(book: LibraryBook): string {
  switch (book.rightsStatus) {
    case 'public_domain':
      return '公共版权经典 · 可站内读'
    case 'open_license':
      return '开放授权 · 可获取阅读'
    case 'official_free':
      return '官方免费 · 外链'
    case 'official_purchase':
      return '需购买 · 官方外链'
    default:
      return '远程条目 · 外链'
  }
}

function isLocalReadableBook(book: LibraryBook): boolean {
  return !!book.filePath && book.availability !== 'remote'
}

function isRemoteLibraryBook(book: LibraryBook): boolean {
  return book.availability === 'remote' || (!book.filePath && !!book.remoteUrl)
}

type LibraryFilter = 'all' | 'readable' | 'remote' | 'unread'
type LibrarySort = 'recent' | 'added' | 'title' | 'author'

const libraryBookCollator = new Intl.Collator('zh-CN', {
  numeric: true,
  sensitivity: 'base',
})

function organizeLibraryBooks(books: LibraryBook[]): LibraryBook[] {
  const filter = libraryFilterSelect.value as LibraryFilter
  const sort = librarySortSelect.value as LibrarySort
  const visible = books.filter((book) => {
    switch (filter) {
      case 'readable':
        return isLocalReadableBook(book)
      case 'remote':
        return isRemoteLibraryBook(book)
      case 'unread':
        return isLocalReadableBook(book) && !book.lastReadAt
      default:
        return true
    }
  })

  return visible.sort((left, right) => {
    if (sort === 'title') {
      return libraryBookCollator.compare(left.title || '', right.title || '')
        || libraryBookCollator.compare(left.author || '', right.author || '')
    }
    if (sort === 'author') {
      return libraryBookCollator.compare(left.author || '', right.author || '')
        || libraryBookCollator.compare(left.title || '', right.title || '')
    }
    const leftTime = sort === 'added' ? left.addedAt : (left.lastReadAt || left.addedAt)
    const rightTime = sort === 'added' ? right.addedAt : (right.lastReadAt || right.addedAt)
    return rightTime - leftTime
      || libraryBookCollator.compare(left.title || '', right.title || '')
  })
}

function setLibraryOrganizeSummary(text: string, enabled = true) {
  libraryFilterSelect.disabled = !enabled
  librarySortSelect.disabled = !enabled
  libraryResultSummary.textContent = text
}

function updateBatchLinkButton(books = libraryBooks) {
  const remoteCount = books.filter(isRemoteLibraryBook).length
  libraryBatchLinkBtn.disabled = remoteCount === 0
  libraryBatchLinkBtn.textContent = remoteCount > 0 ? `批量关联 ${remoteCount}` : '批量关联'
}

interface RankedLinkCandidate {
  book: LibraryBook
  score: number
  reasons: string[]
  warnings: string[]
}

interface BatchLinkQueueEntry {
  remote: LibraryBook
  candidates: RankedLinkCandidate[]
  selectedIndex: number
  sourceSummary: string
  status: 'pending' | 'linked' | 'skipped' | 'error'
  message?: string
}

interface LibraryReadAction {
  key: 'builtin' | 'external' | 'browser' | 'acquire'
  label: string
  title: string
  primary?: boolean
  run: () => Promise<void>
}

function renderLibraryBooks() {
  const books = organizeLibraryBooks(libraryBooks)
  setLibraryOrganizeSummary(books.length === libraryBooks.length
    ? `${books.length} 本`
    : `显示 ${books.length} / ${libraryBooks.length} 本`)
  updateBatchLinkButton(books)

  if (books.length === 0) {
    if (libraryBooks.length > 0) {
      libraryGrid.innerHTML = '<div class="library-state">当前筛选下没有书，换个筛选条件试试。</div>'
    } else if (librarySearchInput.value.trim()) {
      libraryGrid.innerHTML = '<div class="library-state">没有匹配的书</div>'
    } else {
      renderLibraryEmptyState()
    }
    return
  }

  libraryGrid.innerHTML = ''
  for (const b of books) {
    // 远程元数据条目（无本地文件）：封面是 http URL，点击跳官方外链而非站内打开。
    const isRemote = isRemoteLibraryBook(b)
    const card = document.createElement('div')
    card.className = isRemote ? 'book-card book-card-remote' : 'book-card'
    card.dataset.bookId = b.id
    if (b.editionId) card.dataset.editionId = b.editionId
    card.dataset.availability = b.availability || (isRemote ? 'remote' : 'local')
    const cover = document.createElement('div')
    cover.className = 'cover'
    // 书架优先加载缩略图（小图省内存、滚动更流畅），无则回退原封面
    const coverSrc = b.thumbPath || b.coverPath
    if (coverSrc) {
      const img = document.createElement('img')
      img.loading = 'lazy'         // 视口外延迟加载，大书架不一次性拉全部封面
      img.decoding = 'async'
      // 远程封面是来源给的 http(s) URL，直接用；本地封面才经 resource.url 转协议。
      img.src = isRemote ? coverSrc : bridge.resolveFileUrl(coverSrc)
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
    const labels = isRemote ? [remoteStatusLabel(b), language] : [series, language]
    for (const value of labels.filter(Boolean)) {
      const tag = document.createElement('span')
      tag.textContent = value as string
      tags.appendChild(tag)
    }
    const meta = document.createElement('div')
    meta.className = 'meta'
    const lastRead = b.lastReadAt ? `上次阅读 ${new Date(b.lastReadAt).toLocaleDateString('zh-CN')}` : '未读'
    // 远程条目无文件大小：只显示阅读状态；本地条目照常显示体积。
    meta.textContent = b.fileSize
      ? `${Math.max(0.1, b.fileSize / 1024 / 1024).toFixed(1)} MB · ${lastRead}`
      : lastRead
    const actions = document.createElement('div')
    actions.className = 'book-card-actions'
    const sourcesBtn = document.createElement('button')
    sourcesBtn.type = 'button'
    sourcesBtn.className = 'btn btn-subtle'
    sourcesBtn.dataset.action = 'show-sources'
    sourcesBtn.textContent = '来源'
    sourcesBtn.addEventListener('click', (event) => {
      event.stopPropagation()
      void showBookSourcePanel(b)
    })
    actions.appendChild(sourcesBtn)
    if (isRemote) {
      const linkBtn = document.createElement('button')
      linkBtn.type = 'button'
      linkBtn.className = 'btn btn-subtle'
      linkBtn.dataset.action = 'link-remote'
      linkBtn.textContent = '关联本地'
      linkBtn.addEventListener('click', (event) => {
        event.stopPropagation()
        void linkRemoteEntry(b)
      })
      actions.appendChild(linkBtn)
    }
    const readActions = getLibraryReadActions(b)
    for (const readAction of readActions) {
      const readBtn = document.createElement('button')
      readBtn.type = 'button'
      readBtn.className = readAction.primary ? 'btn btn-primary' : 'btn btn-subtle'
      readBtn.dataset.action = `read-${readAction.key}`
      readBtn.textContent = readAction.label
      readBtn.title = readAction.title
      readBtn.addEventListener('click', (event) => {
        event.stopPropagation()
        void readAction.run()
      })
      actions.appendChild(readBtn)
    }
    card.title = [b.title, b.author, series, language, b.description]
      .filter(Boolean)
      .join('\n')
    card.append(cover, t, a)
    if (tags.childElementCount > 0) card.appendChild(tags)
    card.appendChild(meta)
    if (actions.childElementCount > 0) card.appendChild(actions)
    card.addEventListener('click', () => void openDefaultLibraryBookAction(b))
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

async function linkRemoteEntry(remote: LibraryBook) {
  if (!isTauriRuntime()) {
    showError('关联本地书需要 Tauri 桌面窗口')
    return
  }

  try {
    const query = remote.title?.trim() || ''
    const searched = query
      ? await bridge.searchLibraryBooks(query)
      : []
    const allBooks = !query || searched.length < 8
      ? await bridge.listLibraryBooks()
      : []
    const byId = new Map<string, LibraryBook>()
    for (const book of [...searched, ...allBooks]) byId.set(book.id, book)
    const candidates = [...byId.values()].filter((book) =>
      isLocalReadableBook(book) &&
      book.id !== remote.id &&
      book.editionId !== remote.editionId
    )
    const ranked = candidates
      .map((book) => rankLinkCandidate(remote, book))
      .sort(compareRankedLinkCandidates)
      .slice(0, 8)
    const remoteRecords = await bridge.listLibrarySourceRecords(remote.id).catch(() => [])

    showRemoteLinkPanel(remote, ranked, remoteRecords)
  } catch (e: any) {
    showError(`查找本地候选失败：${formatError(e)}`)
  }
}

async function showBatchLinkQueue() {
  if (!isTauriRuntime()) {
    showError('批量关联需要 Tauri 桌面窗口')
    return
  }

  const remotes = libraryBooks.filter(isRemoteLibraryBook)
  if (remotes.length === 0) {
    showError('当前书架没有可关联的远程条目')
    return
  }

  const original = libraryBatchLinkBtn.textContent
  libraryBatchLinkBtn.disabled = true
  libraryBatchLinkBtn.textContent = '整理候选中'
  try {
    const localBooks = (await bridge.listLibraryBooks()).filter(isLocalReadableBook)
    if (localBooks.length === 0) {
      showError('还没有可关联的本地 EPUB，请先导入本地书')
      return
    }
    const entries = await Promise.all(remotes.map(async (remote) => {
      const records = await bridge.listLibrarySourceRecords(remote.id).catch(() => [])
      return {
        remote,
        candidates: rankLocalCandidates(remote, localBooks, 4),
        selectedIndex: 0,
        sourceSummary: formatRemoteSourceSummary(records),
        status: 'pending' as const,
      }
    }))
    renderBatchLinkPanel(entries)
  } catch (e: any) {
    showError(`整理批量关联候选失败：${formatError(e)}`)
  } finally {
    updateBatchLinkButton()
    if (!libraryBatchLinkBtn.disabled) libraryBatchLinkBtn.textContent = original || libraryBatchLinkBtn.textContent
  }
}

function rankLocalCandidates(
  remote: LibraryBook,
  localBooks: LibraryBook[],
  limit: number,
): RankedLinkCandidate[] {
  return localBooks
    .filter((book) =>
      book.id !== remote.id &&
      book.editionId !== remote.editionId
    )
    .map((book) => rankLinkCandidate(remote, book))
    .sort(compareRankedLinkCandidates)
    .slice(0, limit)
}

function renderBatchLinkPanel(entries: BatchLinkQueueEntry[]) {
  libraryGrid.querySelector('.library-batch-panel')?.remove()
  libraryGrid.querySelector('.library-link-panel')?.remove()
  libraryGrid.querySelector('.library-source-panel')?.remove()

  const panel = document.createElement('div')
  panel.className = 'library-batch-panel'

  const header = document.createElement('div')
  header.className = 'library-link-header'
  const title = document.createElement('div')
  title.className = 'library-link-title'
  title.textContent = '批量人工确认'
  const close = document.createElement('button')
  close.type = 'button'
  close.className = 'icon-btn'
  close.textContent = '×'
  close.title = '关闭'
  close.addEventListener('click', () => panel.remove())
  header.append(title, close)

  const pending = entries.filter((entry) => entry.status === 'pending' || entry.status === 'error').length
  const linked = entries.filter((entry) => entry.status === 'linked').length
  const skipped = entries.filter((entry) => entry.status === 'skipped').length
  const subtitle = document.createElement('div')
  subtitle.className = 'library-link-subtitle'
  subtitle.textContent = `${pending} 待确认 · ${linked} 已关联 · ${skipped} 已跳过`
  panel.append(header, subtitle)

  const list = document.createElement('div')
  list.className = 'library-batch-list'
  for (const entry of entries) {
    const row = document.createElement('div')
    row.className = `library-batch-row library-batch-row-${entry.status}`

    const main = document.createElement('div')
    main.className = 'library-batch-main'
    const remoteTitle = document.createElement('div')
    remoteTitle.className = 'library-batch-remote-title'
    remoteTitle.textContent = entry.remote.title || '未命名远程条目'
    const remoteMeta = document.createElement('div')
    remoteMeta.className = 'library-link-candidate-meta'
    remoteMeta.textContent = [
      entry.remote.author,
      entry.sourceSummary,
      remoteStatusLabel(entry.remote),
    ].filter(Boolean).join(' · ') || '远程条目'
    main.append(remoteTitle, remoteMeta)

    if (entry.status === 'linked' || entry.status === 'skipped') {
      const done = document.createElement('div')
      done.className = 'library-batch-status'
      done.textContent = entry.message || (entry.status === 'linked' ? '已关联' : '已跳过')
      main.appendChild(done)
      row.appendChild(main)
      list.appendChild(row)
      continue
    }

    const selected = entry.candidates[entry.selectedIndex]
    if (!selected) {
      const empty = document.createElement('div')
      empty.className = 'library-link-candidate-warning'
      empty.textContent = '没有找到本地候选，可以先跳过'
      main.appendChild(empty)
    } else {
      const picker = document.createElement('select')
      picker.className = 'library-batch-select'
      picker.title = '选择要关联的本地书'
      entry.candidates.forEach((candidate, index) => {
        const option = document.createElement('option')
        option.value = String(index)
        option.textContent = `${candidate.score} · ${candidate.book.title || '未命名'}`
        picker.appendChild(option)
      })
      picker.value = String(entry.selectedIndex)
      picker.addEventListener('change', () => {
        entry.selectedIndex = Number(picker.value)
        renderBatchLinkPanel(entries)
      })

      const match = document.createElement('div')
      match.className = selected.score >= 45
        ? 'library-link-candidate-match'
        : 'library-link-candidate-match library-link-candidate-match-low'
      match.textContent = [
        `匹配 ${selected.score}`,
        ...selected.reasons,
      ].filter(Boolean).join(' · ') || '匹配度低，请人工核对'
      main.append(picker, match)
      if (selected.warnings.length > 0) {
        const warning = document.createElement('div')
        warning.className = 'library-link-candidate-warning'
        warning.textContent = selected.warnings.join(' · ')
        main.appendChild(warning)
      }
    }
    if (entry.status === 'error' && entry.message) {
      const error = document.createElement('div')
      error.className = 'library-link-candidate-warning'
      error.textContent = entry.message
      main.appendChild(error)
    }

    const actions = document.createElement('div')
    actions.className = 'library-batch-actions'
    const link = document.createElement('button')
    link.type = 'button'
    link.className = 'btn btn-primary'
    link.textContent = '关联'
    link.disabled = !selected
    link.addEventListener('click', async () => {
      if (!selected) return
      const caution = selected.score < 45 ? '\n\n匹配度较低，请确认不是同名/相近标题。' : ''
      if (!window.confirm(`将《${entry.remote.title || '远程条目'}》关联到《${selected.book.title || '本地书'}》？${caution}`)) {
        return
      }
      link.disabled = true
      try {
        const linkedBook = await bridge.linkRemoteToLocalLibraryBook(entry.remote.id, selected.book.id)
        applyLinkedRemote(entry.remote, linkedBook)
        removeLibraryCard(entry.remote)
        entry.status = 'linked'
        entry.message = `已关联到《${linkedBook.title || selected.book.title || '本地书'}》`
      } catch (e: any) {
        entry.status = 'error'
        entry.message = `关联失败：${formatError(e)}`
      }
      updateBatchLinkButton()
      renderBatchLinkPanel(entries)
    })

    const skip = document.createElement('button')
    skip.type = 'button'
    skip.className = 'btn btn-subtle'
    skip.textContent = '跳过'
    skip.addEventListener('click', () => {
      entry.status = 'skipped'
      entry.message = '已跳过'
      renderBatchLinkPanel(entries)
    })

    actions.append(link, skip)
    row.append(main, actions)
    list.appendChild(row)
  }
  panel.appendChild(list)

  const footer = document.createElement('div')
  footer.className = 'library-batch-footer'
  const refresh = document.createElement('button')
  refresh.type = 'button'
  refresh.className = 'btn'
  refresh.textContent = '关闭并刷新书架'
  refresh.addEventListener('click', () => {
    panel.remove()
    renderLibraryBooks()
  })
  footer.appendChild(refresh)
  panel.appendChild(footer)

  libraryGrid.prepend(panel)
}

function applyLinkedRemote(remote: LibraryBook, linked: LibraryBook) {
  libraryBooks = [
    linked,
    ...libraryBooks.filter((item) =>
      item.id !== remote.id &&
      item.editionId !== remote.editionId &&
      item.id !== linked.id
    ),
  ]
}

function removeLibraryCard(book: LibraryBook) {
  for (const card of libraryGrid.querySelectorAll<HTMLElement>('.book-card')) {
    if (card.dataset.bookId === book.id || card.dataset.editionId === book.editionId) {
      card.remove()
    }
  }
}

function rankLinkCandidate(remote: LibraryBook, book: LibraryBook): RankedLinkCandidate {
  let score = 0
  const reasons: string[] = []
  const warnings: string[] = []

  const title = scoreTextMatch('标题', remote.title, book.title, 64, 46, 32)
  score += title.score
  if (title.reason) reasons.push(title.reason)

  const author = scoreTextMatch('作者', remote.author, book.author, 24, 14, 10)
  score += author.score
  if (author.reason) reasons.push(author.reason)

  const series = scoreTextMatch('系列', remote.series, book.series, 16, 10, 8)
  score += series.score
  if (series.reason) reasons.push(series.reason)

  if (remote.language && book.language) {
    if (remote.language.toLowerCase() === book.language.toLowerCase()) {
      score += 8
      reasons.push('语言一致')
    } else {
      warnings.push(`语言不同：${formatLanguageLabel(remote.language)} / ${formatLanguageLabel(book.language)}`)
    }
  }

  if (
    remote.seriesIndex !== undefined &&
    remote.seriesIndex !== null &&
    book.seriesIndex !== undefined &&
    book.seriesIndex !== null
  ) {
    if (Number(remote.seriesIndex) === Number(book.seriesIndex)) {
      score += 6
      reasons.push('卷号一致')
    } else {
      warnings.push(`卷号不同：${remote.seriesIndex} / ${book.seriesIndex}`)
    }
  }

  if (score < 30) warnings.push('弱匹配，建议只在确认同一本书时关联')
  return {
    book,
    score: Math.min(100, Math.round(score)),
    reasons,
    warnings,
  }
}

function compareRankedLinkCandidates(a: RankedLinkCandidate, b: RankedLinkCandidate): number {
  if (b.score !== a.score) return b.score - a.score
  const bRecent = b.book.lastReadAt || b.book.addedAt || 0
  const aRecent = a.book.lastReadAt || a.book.addedAt || 0
  if (bRecent !== aRecent) return bRecent - aRecent
  return (a.book.title || '').localeCompare(b.book.title || '', 'zh-CN')
}

function scoreTextMatch(
  label: string,
  remoteValue: string | undefined,
  localValue: string | undefined,
  exactScore: number,
  containsScore: number,
  overlapScore: number,
): { score: number; reason: string } {
  const remote = normalizeMatchText(remoteValue)
  const local = normalizeMatchText(localValue)
  if (!remote || !local) return { score: 0, reason: '' }
  if (remote === local) return { score: exactScore, reason: `${label}一致` }
  if (remote.includes(local) || local.includes(remote)) {
    return { score: containsScore, reason: `${label}相近` }
  }
  const overlap = matchTokenOverlap(remote, local)
  const score = Math.round(overlapScore * overlap)
  return score >= Math.max(6, overlapScore * 0.45)
    ? { score, reason: `${label}部分匹配` }
    : { score, reason: '' }
}

function normalizeMatchText(value?: string): string {
  return (value || '')
    .normalize('NFKC')
    .toLowerCase()
    .replace(/[\p{P}\p{S}\s_]+/gu, '')
}

function matchTokenOverlap(a: string, b: string): number {
  const left = new Set(matchTokens(a))
  const right = new Set(matchTokens(b))
  if (left.size === 0 || right.size === 0) return 0
  let common = 0
  for (const token of left) if (right.has(token)) common += 1
  return common / Math.max(1, Math.min(left.size, right.size))
}

function matchTokens(value: string): string[] {
  const ascii = value.match(/[a-z0-9]+/g) || []
  const cjk = [...value.replace(/[a-z0-9]+/g, '')].filter(Boolean)
  return [...new Set([...ascii, ...cjk])]
}

function formatRemoteSourceSummary(records: LibrarySourceRecord[]): string {
  return records
    .map((record) => {
      const kind = sourceKindLabel(record.sourceKind)
      return kind ? `${record.sourceName} / ${kind}` : record.sourceName
    })
    .filter(Boolean)
    .slice(0, 3)
    .join(' · ')
}

function showRemoteLinkPanel(
  remote: LibraryBook,
  candidates: RankedLinkCandidate[],
  remoteRecords: LibrarySourceRecord[],
) {
  libraryGrid.querySelector('.library-link-panel')?.remove()

  const panel = document.createElement('div')
  panel.className = 'library-link-panel'

  const header = document.createElement('div')
  header.className = 'library-link-header'
  const title = document.createElement('div')
  title.className = 'library-link-title'
  title.textContent = '关联本地书'
  const close = document.createElement('button')
  close.type = 'button'
  close.className = 'icon-btn'
  close.textContent = '×'
  close.title = '关闭'
  close.addEventListener('click', () => panel.remove())
  header.append(title, close)

  const subtitle = document.createElement('div')
  subtitle.className = 'library-link-subtitle'
  const sourceSummary = formatRemoteSourceSummary(remoteRecords)
  subtitle.textContent = [
    `远程条目：${remote.title || '未命名'}`,
    sourceSummary ? `来源：${sourceSummary}` : '',
  ].filter(Boolean).join(' · ')

  panel.append(header, subtitle)

  if (candidates.length === 0) {
    const empty = document.createElement('div')
    empty.className = 'library-link-empty'
    empty.textContent = '没有找到可关联的本地书，请先导入 EPUB。'
    panel.appendChild(empty)
  } else {
    const list = document.createElement('div')
    list.className = 'library-link-candidates'
    for (const entry of candidates) {
      const candidate = entry.book
      const row = document.createElement('div')
      row.className = 'library-link-candidate'

      const main = document.createElement('div')
      main.className = 'library-link-candidate-main'
      const name = document.createElement('div')
      name.className = 'library-link-candidate-title'
      name.textContent = candidate.title || '未命名'
      const meta = document.createElement('div')
      meta.className = 'library-link-candidate-meta'
      meta.textContent = [candidate.author, formatSeriesLabel(candidate), formatLanguageLabel(candidate.language)]
        .filter(Boolean)
        .join(' · ') || '本地 EPUB'
      const match = document.createElement('div')
      match.className = entry.score >= 45
        ? 'library-link-candidate-match'
        : 'library-link-candidate-match library-link-candidate-match-low'
      match.textContent = [
        `匹配 ${entry.score}`,
        ...entry.reasons,
      ].filter(Boolean).join(' · ') || '匹配度低，请人工核对'
      main.append(name, meta, match)
      if (entry.warnings.length > 0) {
        const warning = document.createElement('div')
        warning.className = 'library-link-candidate-warning'
        warning.textContent = entry.warnings.join(' · ')
        main.appendChild(warning)
      }

      const button = document.createElement('button')
      button.type = 'button'
      button.className = 'btn btn-primary'
      button.textContent = '关联'
      button.addEventListener('click', async () => {
        const caution = entry.score < 45 ? '\n\n匹配度较低，请确认不是同名/相近标题。' : ''
        if (!window.confirm(`将《${remote.title || '远程条目'}》关联到《${candidate.title || '本地书'}》？${caution}`)) {
          return
        }
        button.disabled = true
        try {
          const linked = await bridge.linkRemoteToLocalLibraryBook(remote.id, candidate.id)
          applyLinkedRemote(remote, linked)
          renderLibraryBooks()
          prependLibraryLinkSummary(remote, linked)
        } catch (e: any) {
          button.disabled = false
          showError(`关联失败：${formatError(e)}`)
        }
      })

      row.append(main, button)
      list.appendChild(row)
    }
    panel.appendChild(list)
  }

  libraryGrid.prepend(panel)
}

async function showBookSourcePanel(book: LibraryBook) {
  libraryGrid.querySelector('.library-source-panel')?.remove()

  try {
    const records = await bridge.listLibrarySourceRecords(book.id)
    renderBookSourcePanel(book, records)
  } catch (e: any) {
    showError(`读取来源记录失败：${formatError(e)}`)
  }
}

function renderBookSourcePanel(book: LibraryBook, records: LibrarySourceRecord[]) {
  libraryGrid.querySelector('.library-source-panel')?.remove()

  const panel = document.createElement('div')
  panel.className = 'library-source-panel'

  const header = document.createElement('div')
  header.className = 'library-link-header'
  const title = document.createElement('div')
  title.className = 'library-link-title'
  title.textContent = '来源记录'
  const close = document.createElement('button')
  close.type = 'button'
  close.className = 'icon-btn'
  close.textContent = '×'
  close.title = '关闭'
  close.addEventListener('click', () => panel.remove())
  header.append(title, close)

  const subtitle = document.createElement('div')
  subtitle.className = 'library-link-subtitle'
  subtitle.textContent = book.title || '未命名'
  panel.append(header, subtitle)

  if (records.length === 0) {
    const empty = document.createElement('div')
    empty.className = 'library-link-empty'
    empty.textContent = book.remoteUrl
      ? '该条目还没有独立来源记录，可先通过在线找书或手动关联补齐。'
      : '该本地书尚未关联在线来源。'
    panel.appendChild(empty)
  } else {
    const list = document.createElement('div')
    list.className = 'library-source-records'
    for (const record of records) {
      const row = document.createElement('div')
      row.className = 'library-source-record'

      const main = document.createElement('div')
      main.className = 'library-source-record-main'
      const name = document.createElement('div')
      name.className = 'library-source-record-title'
      name.textContent = record.sourceName || record.sourceId

      const meta = document.createElement('div')
      meta.className = 'library-source-record-meta'
      meta.textContent = [
        sourceKindLabel(record.sourceKind),
        sourceRightsLabel(record.rightsStatus),
        sourceAvailabilityLabel(record.availability),
        record.remoteId ? `ID ${record.remoteId}` : '',
        record.lastCheckedAt ? `检查 ${new Date(record.lastCheckedAt).toLocaleDateString('zh-CN')}` : '',
      ].filter(Boolean).join(' · ')

      main.append(name, meta)
      if (record.remoteUrl) {
        const url = document.createElement('div')
        url.className = 'library-source-record-url'
        url.textContent = record.remoteUrl
        main.appendChild(url)
      }
      if (record.acquisitionUrl) {
        const acq = document.createElement('div')
        acq.className = 'library-source-record-url'
        acq.textContent = `获取：${record.acquisitionUrl}`
        main.appendChild(acq)
      }

      if (record.remoteUrl) {
        const button = document.createElement('button')
        button.type = 'button'
        button.className = 'btn btn-subtle'
        button.textContent = '外链'
        button.addEventListener('click', async () => {
          button.disabled = true
          try {
            await bridge.openExternal(record.remoteUrl!)
          } catch (e: any) {
            showError(`打开来源外链失败：${formatError(e)}`)
          } finally {
            button.disabled = false
          }
        })
        row.append(main, button)
      } else {
        row.appendChild(main)
      }
      list.appendChild(row)
    }
    panel.appendChild(list)
  }

  libraryGrid.prepend(panel)
}

function sourceKindLabel(kind?: string): string {
  switch (kind) {
    case 'metadata':
      return '元数据'
    case 'catalog':
      return '目录'
    case 'public_domain':
      return '公共版权'
    default:
      return kind || '来源'
  }
}

function sourceRightsLabel(status?: string): string {
  switch (status) {
    case 'public_domain':
      return '公共版权'
    case 'official_free':
      return '官方免费'
    case 'official_purchase':
      return '官方购买'
    case 'user_owned':
      return '自有资产'
    default:
      return status || '授权未知'
  }
}

function sourceAvailabilityLabel(availability?: string): string {
  switch (availability) {
    case 'local':
      return '本地可读'
    case 'cached':
      return '已缓存'
    case 'remote':
      return '远程条目'
    case 'missing':
      return '缺失'
    default:
      return availability || ''
  }
}

function prependLibraryLinkSummary(remote: LibraryBook, linked: LibraryBook) {
  const msg = document.createElement('div')
  msg.className = 'library-import-summary'
  msg.textContent = `已关联：${remote.title || '远程条目'} → ${linked.title || '本地书'}`
  libraryGrid.prepend(msg)
}

// 远程条目无本地正文：按版权红线只跳官方/来源外链，不在站内呈现正文。
function canAcquireForBuiltInReader(book: LibraryBook): boolean {
  return (
    isRemoteLibraryBook(book) &&
    (book.rightsStatus === 'public_domain' ||
      (book.rightsStatus === 'open_license' && !!book.acquisitionUrl))
  )
}

function getLibraryReadActions(book: LibraryBook): LibraryReadAction[] {
  const actions: LibraryReadAction[] = []
  const isRemote = isRemoteLibraryBook(book)
  const isReadableAsset = !isRemote && book.availability !== 'missing'

  if (isReadableAsset) {
    actions.push({
      key: 'builtin',
      label: '内置',
      title: '用内置阅读器打开',
      run: () => openLibraryBook(book),
    })
  }

  if (book.filePath && book.availability !== 'remote') {
    actions.push({
      key: 'external',
      label: '外部',
      title: '用系统默认本地阅读器打开',
      run: () => openExternalLibraryBook(book),
    })
  }

  if (canAcquireForBuiltInReader(book)) {
    actions.push({
      key: 'acquire',
      label: '获取',
      title: '获取公共版权正文并用内置阅读器打开',
      run: () => acquireAndOpenRemoteBook(book),
    })
  }

  if (book.remoteUrl) {
    actions.push({
      key: 'browser',
      label: '浏览器',
      title: '打开官方页面',
      run: () => openRemoteOfficialPage(book),
    })
  }

  const primary = selectPreferredLibraryReadAction(actions)
  for (const action of actions) {
    action.primary = action === primary
  }
  return actions
}

function selectPreferredLibraryReadAction(actions: LibraryReadAction[]): LibraryReadAction | undefined {
  if (actions.length === 0) return undefined
  const preference = readLibraryReadPreference()
  if (preference === 'builtin') {
    return actions.find((item) => item.key === 'builtin' || item.key === 'acquire') ?? actions[0]
  }
  if (preference === 'browser') {
    return actions.find((item) => item.key === 'browser') ?? actions[0]
  }
  if (preference === 'external') {
    return actions.find((item) => item.key === 'external') ?? actions[0]
  }
  return actions.find((item) => item.key === 'builtin' || item.key === 'acquire' || item.key === 'browser') ?? actions[0]
}

async function openDefaultLibraryBookAction(book: LibraryBook) {
  const actions = getLibraryReadActions(book)
  const action = actions.find((item) => item.primary) ?? selectPreferredLibraryReadAction(actions)
  if (!action) {
    showError('该条目还没有可用的阅读方式')
    return
  }
  await action.run()
}

async function openRemoteOfficialPage(book: LibraryBook) {
  if (!book.remoteUrl) {
    showError('该条目没有可跳转的官方链接')
    return
  }
  try {
    await bridge.openExternal(book.remoteUrl)
  } catch (e: any) {
    showError(`打开官方链接失败：${formatError(e)}`)
  }
}

async function openExternalLibraryBook(book: LibraryBook) {
  if (!book.filePath) {
    showError('该条目没有可交给外部阅读器的本地文件')
    return
  }
  try {
    await bridge.openPathExternal(book.filePath)
  } catch (e: any) {
    showError(`打开外部阅读器失败：${formatError(e)}`)
  }
}

async function acquireAndOpenRemoteBook(book: LibraryBook) {
  libraryGrid.innerHTML = `<div class="library-state">正在获取《${book.title || '未命名'}》正文…</div>`
  try {
    const acquired =
      book.rightsStatus === 'open_license'
        ? await bridge.opdsDownloadEpub(book.editionId || book.id, book.acquisitionUrl)
        : await bridge.acquireRemoteLibraryBook(book.id)
    libraryBooks = libraryBooks.map((item) =>
      item.id === book.id || item.editionId === book.editionId ? acquired : item,
    )
    renderLibraryBooks()
    await openAcquiredLibraryBook(acquired)
  } catch (e: any) {
    renderLibraryBooks()
    showError(`获取公共版权正文失败：${formatError(e)}`)
  }
}

async function openAcquiredLibraryBook(book: LibraryBook) {
  if (readLibraryReadPreference() === 'external' && book.filePath) {
    await openExternalLibraryBook(book)
    return
  }
  await openLibraryBook(book)
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
    showError(`打开失败：${formatError(e)}`)
  }
}

// ── OPDS v0.6 ──

// 监听 OPDS 面板打开 → 刷新源列表；添加源按钮 → 添加源
libraryOpdsPanel.addEventListener('toggle', () => {
  if (libraryOpdsPanel.open) {
    void refreshOpdsSources()
  }
})
opdsAddSourceBtn.addEventListener('click', () => void addOpdsSource())
opdsSourceUrlInput.addEventListener('keydown', (e) => {
  if (e.key === 'Enter') void addOpdsSource()
})
opdsFeedBackBtn.addEventListener('click', () => hideOpdsFeedView())

opdsFeedIngestAllBtn.addEventListener('click', () => {
  if (opdsFeedCache && opdsSourceCache) {
    void ingestOpdsEntries(opdsSourceCache.id, opdsFeedCache)
  }
})

async function refreshOpdsSources() {
  if (!isTauriRuntime()) return
  try {
    const sources = await bridge.opdsListSources()
    renderOpdsSourceList(sources)
  } catch (e: any) {
    opdsSourceList.innerHTML = `<div class="opds-feed-empty">读取 OPDS 源列表失败：${formatError(e)}</div>`
  }
}

function renderOpdsSourceList(sources: OpdsSource[]) {
  opdsSourceList.innerHTML = ''
  if (sources.length === 0) {
    opdsSourceList.innerHTML = '<div class="opds-feed-empty">暂无 OPDS 书源，请在上方添加。</div>'
    return
  }
  for (const src of sources) {
    const row = document.createElement('div')
    row.className = 'opds-source-row'
    const info = document.createElement('div')
    info.style.minWidth = '0'
    const nameEl = document.createElement('div')
    nameEl.className = 'opds-source-name'
    nameEl.textContent = src.name
    const urlEl = document.createElement('div')
    urlEl.className = 'opds-source-url'
    urlEl.textContent = src.baseUrl || ''
    info.append(nameEl, urlEl)

    const actions = document.createElement('div')
    actions.className = 'opds-source-actions'
    const browseBtn = document.createElement('button')
    browseBtn.className = 'btn'
    browseBtn.textContent = '浏览'
    browseBtn.addEventListener('click', () => {
      void browseOpdsFeed(src.baseUrl || '', src.id, src.name)
    })
    const searchBtn = document.createElement('button')
    searchBtn.className = 'btn'
    searchBtn.textContent = '搜索'
    searchBtn.addEventListener('click', () => {
      const q = librarySearchInput.value.trim()
      if (!q) { showError('请先在书架搜索框输入关键词'); return }
      void searchOpdsFeed(src.id, q, src.name)
    })
    const removeBtn = document.createElement('button')
    removeBtn.className = 'btn'
    removeBtn.textContent = '移除'
    removeBtn.addEventListener('click', () => {
      if (window.confirm(`确定移除 OPDS 源「${src.name}」？`)) {
        void removeOpdsSource(src.id)
      }
    })
    actions.append(browseBtn, searchBtn, removeBtn)
    row.append(info, actions)
    opdsSourceList.appendChild(row)
  }
}

async function addOpdsSource() {
  if (!isTauriRuntime()) {
    showError('需要 Tauri 桌面窗口（请运行 npm run tauri dev）')
    return
  }
  const url = opdsSourceUrlInput.value.trim()
  if (!url) { showError('请输入 OPDS feed URL'); return }
  const name = opdsSourceNameInput.value.trim() || url

  opdsAddSourceBtn.disabled = true
  opdsAddSourceBtn.textContent = '添加中…'
  try {
    await bridge.opdsAddSource(name, url)
    opdsSourceUrlInput.value = ''
    opdsSourceNameInput.value = ''
    await refreshOpdsSources()
  } catch (e: any) {
    showError(`添加 OPDS 源失败：${formatError(e)}`)
  } finally {
    opdsAddSourceBtn.disabled = false
    opdsAddSourceBtn.textContent = '添加 OPDS 源'
  }
}

async function removeOpdsSource(id: string) {
  try {
    await bridge.opdsRemoveSource(id)
    hideOpdsFeedView()
    await refreshOpdsSources()
  } catch (e: any) {
    showError(`移除失败：${formatError(e)}`)
  }
}

async function browseOpdsFeed(url: string, sourceId: string, sourceName: string) {
  if (!isTauriRuntime()) return
  opdsFeedView.hidden = false
  opdsFeedGrid.innerHTML = '<div class="opds-feed-empty">加载中…</div>'
  try {
    const feed = await bridge.opdsBrowseFeed(url)
    opdsFeedCache = feed
    opdsSourceCache = { id: sourceId, name: sourceName, enabled: true, baseUrl: url }
    renderOpdsFeedView(feed, sourceId, sourceName, url)
  } catch (e: any) {
    opdsFeedGrid.innerHTML = `<div class="opds-feed-empty" style="color:var(--error)">加载失败：${formatError(e)}</div>`
  }
}

async function searchOpdsFeed(sourceId: string, query: string, sourceName: string) {
  if (!isTauriRuntime()) return
  opdsFeedView.hidden = false
  opdsFeedGrid.innerHTML = '<div class="opds-feed-empty">搜索中…</div>'
  opdsFeedTitle.textContent = `${sourceName} · 搜索「${query}」`
  try {
    const feed = await bridge.opdsSearchFeed(sourceId, query)
    opdsFeedCache = feed
    renderOpdsFeedView(feed, sourceId, sourceName, '')
  } catch (e: any) {
    opdsFeedGrid.innerHTML = `<div class="opds-feed-empty" style="color:var(--error)">搜索失败：${formatError(e)}</div>`
  }
}

function hideOpdsFeedView() {
  opdsFeedView.hidden = true
  opdsFeedCache = null
  opdsSourceCache = null
  opdsFeedGrid.innerHTML = ''
}

function resolveOpdsEntryUrls(entry: OpdsEntry, resolveUrl: (href: string) => string): OpdsEntry {
  return {
    ...entry,
    coverUrl: entry.coverUrl ? resolveUrl(entry.coverUrl) : entry.coverUrl,
    acquisitionUrl: entry.acquisitionUrl ? resolveUrl(entry.acquisitionUrl) : entry.acquisitionUrl,
    links: entry.links.map((link) => ({ ...link, href: resolveUrl(link.href) })),
  }
}

function resolveOpdsFeedUrls(feed: OpdsFeed, resolveUrl: (href: string) => string): OpdsFeed {
  return {
    ...feed,
    links: feed.links.map((link) => ({ ...link, href: resolveUrl(link.href) })),
    entries: feed.entries.map((entry) => resolveOpdsEntryUrls(entry, resolveUrl)),
  }
}

function renderOpdsFeedView(feed: OpdsFeed, sourceId: string, sourceName: string, feedUrl: string) {
  opdsFeedTitle.textContent = `${sourceName} · ${feed.title || 'OPDS Feed'}`
  opdsFeedGrid.innerHTML = ''

  // Resolve a possibly-relative URL against the feed URL
  const resolveUrl = (href: string): string => {
    if (!feedUrl || /^https?:\/\//i.test(href)) return href
    try { return new URL(href, feedUrl).href } catch { return href }
  }
  const resolvedFeed = resolveOpdsFeedUrls(feed, resolveUrl)
  opdsFeedCache = resolvedFeed

  const pubCount = resolvedFeed.entries.filter((e) => !e.isNavigation).length
  const navCount = resolvedFeed.entries.filter((e) => e.isNavigation).length

  if (pubCount === 0 && navCount === 0) {
    opdsFeedGrid.innerHTML = '<div class="opds-feed-empty">该 feed 没有条目。</div>'
    return
  }

  for (const entry of resolvedFeed.entries) {
    const card = document.createElement('div')
    card.className = entry.isNavigation ? 'opds-feed-card opds-feed-card-nav' : 'opds-feed-card'

    const title = document.createElement('div')
    title.className = 'opds-feed-card-title'
    title.textContent = entry.title || '未命名'

    card.appendChild(title)

    if (entry.author) {
      const author = document.createElement('div')
      author.className = 'opds-feed-card-author'
      author.textContent = entry.author
      card.appendChild(author)
    }

    if (entry.summary) {
      const summary = document.createElement('div')
      summary.className = 'opds-feed-card-summary'
      summary.textContent = entry.summary
      card.appendChild(summary)
    }

    const meta = document.createElement('div')
    meta.className = 'opds-feed-card-meta'
    if (entry.isNavigation) {
      const badge = document.createElement('span')
      badge.className = 'opds-feed-card-badge'
      badge.textContent = '📂 子分类'
      meta.appendChild(badge)
    } else if (entry.acquisitionUrl) {
      const badge = document.createElement('span')
      badge.className = 'opds-feed-card-badge'
      badge.textContent = '📖 可获取 EPUB'
      badge.style.background = 'color-mix(in srgb, var(--accent) 22%, var(--surface))'
      meta.appendChild(badge)
    } else {
      const badge = document.createElement('span')
      badge.className = 'opds-feed-card-badge'
      badge.textContent = '📋 仅元数据'
      badge.style.background = 'color-mix(in srgb, var(--muted) 16%, var(--surface))'
      badge.style.color = 'var(--muted)'
      meta.appendChild(badge)
    }
    card.appendChild(meta)

    const actions = document.createElement('div')
    actions.className = 'opds-feed-card-actions'

    if (entry.isNavigation) {
      // Navigation entry: resolve link and navigate
      const navUrl = resolveUrl(
        entry.links.find(
          (l) => l.rel === 'subsection' || l.rel === 'alternate' || l.href
        )?.href || ''
      )
      if (navUrl) {
        const navBtn = document.createElement('button') as HTMLButtonElement
        navBtn.className = 'btn'
        navBtn.textContent = '进入'
        navBtn.addEventListener('click', () => {
          void browseOpdsFeed(navUrl, sourceId, sourceName)
        })
        actions.appendChild(navBtn)
        card.style.cursor = 'pointer'
        card.addEventListener('click', (e) => {
          if (e.target === navBtn || navBtn.contains(e.target as Node)) return
          void browseOpdsFeed(navUrl, sourceId, sourceName)
        })
      }
    } else {
      // Publication entry: add to library
      const addBtn = document.createElement('button') as HTMLButtonElement
      addBtn.className = 'btn btn-primary'
      addBtn.textContent = '加入书架'
      addBtn.addEventListener('click', async () => {
        addBtn.disabled = true
        addBtn.textContent = '添加中…'
        try {
          const singleFeed: OpdsFeed = { title: entry.title, entries: [entry], links: [] }
          const books = await bridge.opdsIngestEntries(sourceId, { ...singleFeed })
          if (books.length > 0) {
            addBtn.textContent = '已加入 ✓'
            await refreshLibraryBooks()
          }
        } catch (e: any) {
          addBtn.textContent = '失败'
          showError(`加入书架失败：${formatError(e)}`)
        } finally {
          addBtn.disabled = true
        }
      })
      actions.appendChild(addBtn)

      // Download button for open_license EPUB entries
      const acqUrl = entry.acquisitionUrl
      if (acqUrl) {
        const dlBtn = document.createElement('button') as HTMLButtonElement
        dlBtn.className = 'btn btn-primary'
        dlBtn.textContent = '获取并阅读'
        dlBtn.addEventListener('click', async () => {
          dlBtn.disabled = true
          dlBtn.textContent = '获取中…'
          try {
            const acquired = await acquireOpdsEntry(sourceId, entry, resolveUrl(acqUrl))
            dlBtn.textContent = '已获取 ✓'
            addBtn.disabled = true
            addBtn.textContent = '已在书架'
            await refreshLibraryBooks()
            if (acquired) await openAcquiredLibraryBook(acquired)
          } catch (e: any) {
            dlBtn.textContent = '获取失败'
            showError(`获取 EPUB 失败：${formatError(e)}`)
          } finally {
            dlBtn.disabled = true
          }
        })
        actions.appendChild(dlBtn)
      }

      // External link if available
      const siteUrl = resolveUrl(
        entry.links.find((l) => l.rel === 'alternate')?.href || ''
      )
      if (siteUrl) {
        const extBtn = document.createElement('button') as HTMLButtonElement
        extBtn.className = 'btn'
        extBtn.textContent = '打开外链'
        extBtn.addEventListener('click', () => {
          void bridge.openExternal(siteUrl)
        })
        actions.appendChild(extBtn)
      }
    }
    card.appendChild(actions)
    opdsFeedGrid.appendChild(card)
  }
}

async function ingestOpdsEntries(sourceId: string, feed: OpdsFeed) {
  if (!isTauriRuntime()) return
  const pubEntries = feed.entries.filter((e) => !e.isNavigation)
  if (pubEntries.length === 0) {
    showError('当前 feed 没有可加入书架的出版物条目。')
    return
  }
  opdsFeedIngestAllBtn.disabled = true
  opdsFeedIngestAllBtn.textContent = '添加中…'
  try {
    const books = await bridge.opdsIngestEntries(sourceId, feed)
    if (books.length > 0) {
      opdsFeedIngestAllBtn.textContent = `已加入 ${books.length} 本 ✓`
      await refreshLibraryBooks()
    }
  } catch (e: any) {
    showError(`加入书架失败：${formatError(e)}`)
    opdsFeedIngestAllBtn.disabled = false
    opdsFeedIngestAllBtn.textContent = '全部加入书架'
  }
}

async function acquireOpdsEntry(
  sourceId: string,
  entry: OpdsEntry,
  acquisitionUrl: string,
): Promise<LibraryBook> {
  const singleFeed: OpdsFeed = { title: entry.title, entries: [entry], links: [] }
  const books = await bridge.opdsIngestEntries(sourceId, { ...singleFeed })
  if (books.length === 0) throw new Error('落库失败')
  const editionId = books[0].editionId || books[0].id
  if (!editionId) throw new Error('无法获取 edition ID')
  return bridge.opdsDownloadEpub(editionId, acquisitionUrl)
}

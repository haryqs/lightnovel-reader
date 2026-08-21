/**
 * 网页端 ReaderBridge 实现 —— 用 WASM + IndexedDB/OPFS 替代 Tauri shell。
 */
import type {
  Annotation,
  BookInfo,
  ImportOutcome,
  LibraryBook,
  OpenedBook,
  ReadingProgress,
  ReaderBridge,
  SyncCredential,
  SyncStatus,
  TextAnchor,
} from '../platform/protocol'
import * as storage from './web-storage'
import { initReadingCore, parseEpubMetadata, getChapterHtml } from './reading-core-wasm'

// ---- 状态 ----

let _wasmReady = false
let _wasmInit: Promise<unknown> | null = null
let _currentBookId = ''
// eslint-disable-next-line @typescript-eslint/no-unused-vars
void (_currentBookId)
let _currentBookData: Uint8Array | null = null

function ensureWasm(): Promise<unknown> {
  if (!_wasmInit) {
    _wasmInit = initReadingCore().then(() => { _wasmReady = true })
  }
  return _wasmInit
}

function platformErr(method: string, msg?: string) {
  return { code: 'platformError' as const, message: msg || `${method}: 浏览器端不可用` }
}

function notFound(msg: string) {
  return { code: 'notFound' as const, message: msg }
}

// ---- Bridge ----

export const webBridge: ReaderBridge = {
  // -- 打开书 --

  async openBookFromBytes(data: Uint8Array): Promise<BookInfo> {
    await ensureWasm()
    if (!_wasmReady) throw platformErr('book.open', 'WASM 引擎未就绪')

    const json = parseEpubMetadata(data)
    const parsed = JSON.parse(json)
    if (parsed.error) throw { code: 'parseError' as const, message: parsed.error }

    const bookId = await computeBookId(data.buffer as ArrayBuffer)
    _currentBookId = bookId
    _currentBookData = data

    const info: BookInfo = {
      metadata: parsed.metadata || { title: '未命名' },
      toc: parsed.toc || [],
      spine: parsed.spine || [],
    }

    await storage.saveBook({
      bookId,
      title: parsed.metadata?.title || '未命名',
      author: parsed.metadata?.author,
      language: parsed.metadata?.language,
      description: parsed.metadata?.description,
      series: parsed.metadata?.series,
      seriesIndex: parsed.metadata?.series_index,
      tocJson: JSON.stringify(info.toc),
      spineJson: JSON.stringify(info.spine),
      addedAt: Date.now(),
      lastReadAt: Date.now(),
    })
    await storage.saveEpubFile(bookId, data.buffer as ArrayBuffer)

    return info
  },

  async openBookFromPath(_path: string): Promise<OpenedBook> {
    throw platformErr('book.openPath')
  },

  async closeBook(): Promise<void> {
    _currentBookId = ''
    _currentBookData = null
  },

  // -- 章节 --

  async getChapter(href: string): Promise<string> {
    await ensureWasm()
    if (!_wasmReady) throw platformErr('chapter.get', 'WASM 未就绪')
    if (!_currentBookData) throw notFound('未打开任何书')

    const html = getChapterHtml(_currentBookData, href)
    if (html.startsWith('<p>') && html.includes('失败')) {
      throw { code: 'parseError' as const, message: html }
    }
    return html
  },

  // -- 书库 --

  async listLibraryBooks(): Promise<LibraryBook[]> {
    const books = await storage.listBooks()
    return books.map(b => ({
      id: b.bookId,
      title: b.title,
      addedAt: b.addedAt,
    }))
  },

  searchLibraryBooks: () => Promise.resolve([]),
  listLibrarySourceRecords: () => Promise.resolve([]),
  searchRemoteLibraryBooks: () => Promise.resolve([]),
  searchRemoteLibraryBooksFromSource: () => Promise.resolve([]),
  acquireRemoteLibraryBook: (_id: string) => Promise.reject(platformErr('library.acquireRemote')),
  linkRemoteToLocalLibraryBook: () => Promise.reject(platformErr('library.linkRemote')),
  openLibraryBook: (_id: string) => Promise.reject(platformErr('library.open')),
  touchLibraryLastRead: () => Promise.resolve(),
  listCalibreBooks: () => Promise.resolve([]),
  importLibraryBook: (): Promise<ImportOutcome> => Promise.reject(platformErr('library.import')),
  importLibraryBookFromBytes: (): Promise<ImportOutcome> => Promise.reject(platformErr('library.importBytes')),

  // -- 标注/进度 --

  async saveAnnotation(ann: Annotation): Promise<void> {
    const text = ann.locator.anchor.exact
    const cfi = `${ann.locator.chapterHref}#${ann.locator.anchor.start}`
    await storage.saveAnnotation({
      id: ann.id || crypto.randomUUID(),
      bookId: ann.bookId,
      cfi,
      text: text || '',
      note: ann.note || '',
      color: ann.color || 'yellow',
      createdAt: ann.createdAt,
    })
  },

  async listAnnotations(bookId: string): Promise<Annotation[]> {
    const anns = await storage.listAnnotations(bookId)
    return anns.map(a => ({
      id: a.id,
      bookId: a.bookId,
      kind: 'highlight' as const,
      color: a.color,
      note: a.note,
      locator: {
        chapterHref: '',
        anchor: {
          start: 0,
          end: a.text.length,
          exact: a.text,
          prefix: '',
          suffix: '',
        } as TextAnchor,
      },
      createdAt: a.createdAt,
      updatedAt: a.createdAt,
    }))
  },

  async deleteAnnotation(id: string): Promise<void> {
    await storage.deleteAnnotation(id)
  },

  async saveProgress(p: ReadingProgress): Promise<void> {
    await storage.saveProgress({
      bookId: p.bookId,
      cfi: p.chapterHref,
      percentage: p.chapterProgress * 100,
      updatedAt: p.updatedAt,
    })
  },

  async getProgress(bookId: string): Promise<ReadingProgress | null> {
    const p = await storage.getProgress(bookId)
    if (!p) return null
    return {
      bookId: p.bookId,
      chapterHref: p.cfi,
      chapterProgress: p.percentage / 100,
      percentage: p.percentage / 100,
      updatedAt: p.updatedAt,
    }
  },

  // -- 插件（web 端不支持）--

  selectPluginPackagePath: () => Promise.reject(platformErr('plugin')),
  inspectPluginPackage: () => Promise.reject(platformErr('plugin')),
  installPluginPackage: () => Promise.reject(platformErr('plugin')),
  listInstalledPlugins: () => Promise.resolve([]),
  setPluginEnabled: () => Promise.reject(platformErr('plugin')),
  uninstallPlugin: () => Promise.reject(platformErr('plugin')),
  loadPluginRepositoryIndex: () => Promise.reject(platformErr('plugin')),
  inspectRepositoryPluginPackage: () => Promise.reject(platformErr('plugin')),
  installRepositoryPluginPackage: () => Promise.reject(platformErr('plugin')),
  testPluginFlow: () => Promise.reject(platformErr('plugin')),
  listPluginSources: () => Promise.resolve([]),
  searchPluginSource: () => Promise.reject(platformErr('plugin')),
  getPluginSourceBook: () => Promise.reject(platformErr('plugin')),
  getPluginSourceChapter: () => Promise.reject(platformErr('plugin')),
  collectPluginSourceBook: () => Promise.reject(platformErr('plugin')),
  acquirePluginSourceBook: () => Promise.reject(platformErr('plugin')),

  // -- 兜底 --

  resolveFileUrl: (path: string) => path,
  openExternal: async (url: string) => { window.open(url, '_blank', 'noopener') },
  openPathExternal: () => Promise.reject(platformErr('shell')),
  checkAppUpdate: () => Promise.resolve(null),
  installAppUpdate: () => Promise.reject(platformErr('appUpdate')),

  // -- OPDS --

  opdsAddSource: () => Promise.reject(platformErr('opds')),
  opdsRemoveSource: () => Promise.reject(platformErr('opds')),
  opdsListSources: () => Promise.resolve([]),
  opdsBrowseFeed: () => Promise.reject(platformErr('opds')),
  opdsSearchFeed: () => Promise.reject(platformErr('opds')),
  opdsIngestEntries: () => Promise.reject(platformErr('opds')),
  opdsDownloadEpub: () => Promise.reject(platformErr('opds')),

  // -- sync v1 (Phase 2) --
  syncPair: (code: string) => syncPairImpl(code),
  syncStatus: () => Promise.resolve(syncStatusImpl()),
  syncNow: async () => {}, // 网页端暂不支持主动同步
  syncUnpair: async () => { clearSyncCredential() },
}

// ---- 同步辅助 ----

const SYNC_STORAGE_KEY = 'lnr-sync-cred'

function getSyncCredential(): SyncCredential | null {
  try {
    const raw = localStorage.getItem(SYNC_STORAGE_KEY)
    return raw ? JSON.parse(raw) : null
  } catch { return null }
}

function saveSyncCredential(cred: SyncCredential): void {
  localStorage.setItem(SYNC_STORAGE_KEY, JSON.stringify(cred))
}

function clearSyncCredential(): void {
  localStorage.removeItem(SYNC_STORAGE_KEY)
}

async function syncPairImpl(code: string): Promise<SyncCredential> {
  const cred = getSyncCredential()
  const serverUrl = cred?.serverUrl || ''
  if (!serverUrl) throw platformErr('sync.pair', '未配置同步服务器地址')

  const res = await fetch(`${serverUrl}/pair/join`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ pairing_code: code, device_name: 'web' }),
  })
  if (!res.ok) throw { code: 'forbidden' as const, message: '配对码无效' }
  const data = await res.json()
  const newCred: SyncCredential = { libraryId: data.library_id, token: data.token, serverUrl }
  saveSyncCredential(newCred)
  return newCred
}

function syncStatusImpl(): SyncStatus {
  const cred = getSyncCredential()
  return {
    paired: cred !== null,
    lastSyncAt: null,
    pendingChanges: 0,
    libraryId: cred?.libraryId || null,
  }
}

// ---- 辅助 ----

async function computeBookId(data: ArrayBuffer): Promise<string> {
  const hashBuffer = await crypto.subtle.digest('SHA-256', data)
  return Array.from(new Uint8Array(hashBuffer))
    .map(b => b.toString(16).padStart(2, '0'))
    .join('')
    .slice(0, 32)
}

// Tauri 桌面壳的 ReaderBridge 实现:协议方法 → Tauri command 的唯一映射点。
// @tauri-apps/* 只允许出现在 src/platform/ 内(scripts/check-arch.mjs 强制)。
import { convertFileSrc, invoke } from '@tauri-apps/api/core'
export { invoke }
import { open } from '@tauri-apps/plugin-dialog'
import { openPath, openUrl } from '@tauri-apps/plugin-opener'
import { relaunch } from '@tauri-apps/plugin-process'
import { check, type DownloadEvent, type Update } from '@tauri-apps/plugin-updater'
import { isBridgeError } from './protocol'
import type {
  Annotation,
  AppUpdateInfo,
  AppUpdateInstallProgress,
  BridgeError,
  BookInfo,
  CalibreBook,
  ImportOutcome,
  InstalledPlugin,
  LibraryBook,
  LibrarySourceRecord,
  OpdsFeed,
  OpdsSource,
  OpenedBook,
  PluginInstallPreview,
  PluginBookDetail,
  PluginChapterContent,
  PluginRepositoryCatalog,
  PluginPackageSignature,
  PluginSearchPage,
  PluginSourceDescriptor,
  PluginTestFlowResult,
  ReaderBridge,
  ReadingProgress,
  RemoteLibrarySource,
  UserDataBackupResult,
  UserDataBackupInspection,
} from './protocol'

let pendingAppUpdate: Update | null = null
let activeAppUpdateInstall: Promise<void> | null = null
const appUpdateProgressListeners = new Set<(progress: AppUpdateInstallProgress) => void>()

export const isTauriRuntime = () =>
  Boolean((window as any).__TAURI_INTERNALS__)

function bridgeError(code: BridgeError['code'], message: string, details?: unknown): BridgeError {
  return {
    code,
    message,
    details: details === undefined ? undefined : String(details),
  }
}

async function openUrlExternal(url: string): Promise<void> {
  if (!url || !url.trim()) {
    throw bridgeError('invalidArgument', 'URL is required')
  }
  try {
    await openUrl(url)
  } catch (err) {
    throw bridgeError('platformError', '打开外部链接失败', err instanceof Error ? err.message : err)
  }
}

async function openLocalPathExternal(path: string): Promise<void> {
  if (!path || !path.trim()) {
    throw bridgeError('invalidArgument', 'path is required')
  }
  try {
    await openPath(path)
  } catch (err) {
    throw bridgeError('platformError', '打开本地文件失败', err instanceof Error ? err.message : err)
  }
}

async function closePendingAppUpdate(): Promise<void> {
  const pending = pendingAppUpdate
  pendingAppUpdate = null
  if (pending) await pending.close().catch(() => undefined)
}

async function checkAppUpdate(): Promise<AppUpdateInfo | null> {
  if (activeAppUpdateInstall) {
    throw bridgeError('platformError', '应用更新正在安装，请等待应用重启')
  }
  await closePendingAppUpdate()
  try {
    const update = await check()
    pendingAppUpdate = update
    if (!update) return null
    return {
      currentVersion: update.currentVersion,
      version: update.version,
      date: update.date,
      body: update.body,
    }
  } catch (err) {
    throw bridgeError('platformError', '检查应用更新失败', err instanceof Error ? err.message : err)
  }
}

function emitAppUpdateProgress(progress: AppUpdateInstallProgress): void {
  for (const listener of appUpdateProgressListeners) {
    try {
      listener(progress)
    } catch (err) {
      console.error('应用更新进度监听器失败', err)
    }
  }
}

async function runAppUpdateInstall(): Promise<void> {
  let update = pendingAppUpdate
  let downloadedBytes = 0
  let totalBytes: number | undefined
  try {
    if (!update) {
      update = await check()
      pendingAppUpdate = update
    }
    if (!update) throw bridgeError('notFound', '当前没有可安装的应用更新')
    const onDownloadEvent = (event: DownloadEvent) => {
      if (event.event === 'Started') {
        downloadedBytes = 0
        totalBytes = event.data.contentLength
        emitAppUpdateProgress({ stage: 'downloading', downloadedBytes, totalBytes })
        return
      }
      if (event.event === 'Progress') {
        downloadedBytes += event.data.chunkLength
        emitAppUpdateProgress({ stage: 'downloading', downloadedBytes, totalBytes })
        return
      }
      emitAppUpdateProgress({ stage: 'installing', downloadedBytes, totalBytes })
    }
    await update.downloadAndInstall(onDownloadEvent)
    pendingAppUpdate = null
    await relaunch()
  } catch (err) {
    if (pendingAppUpdate === update) pendingAppUpdate = null
    if (isBridgeError(err)) throw err
    throw bridgeError('platformError', '下载或安装应用更新失败', err instanceof Error ? err.message : err)
  } finally {
    if (update && pendingAppUpdate !== update) {
      await update.close().catch(() => undefined)
    }
  }
}

function installAppUpdate(onProgress?: (progress: AppUpdateInstallProgress) => void): Promise<void> {
  if (onProgress) appUpdateProgressListeners.add(onProgress)
  if (!activeAppUpdateInstall) {
    activeAppUpdateInstall = runAppUpdateInstall().finally(() => {
      activeAppUpdateInstall = null
      appUpdateProgressListeners.clear()
    })
  }
  const operation = activeAppUpdateInstall
  return operation.finally(() => {
    if (onProgress) appUpdateProgressListeners.delete(onProgress)
  })
}

export const tauriBridge: ReaderBridge = {
  openBookFromBytes: (data) => invoke<BookInfo>('open_book_bytes', { data }),
  openBookFromPath: (path) => invoke<OpenedBook>('open_book_path', { path }),
  closeBook: () => invoke('close_book'),
  getChapter: (href) => invoke<string>('get_chapter', { href }),
  listCalibreBooks: (library) =>
    invoke<CalibreBook[]>('list_calibre_books', { library }),
  importLibraryBook: (path) =>
    invoke<ImportOutcome>('library_import', { path }),
  importLibraryBookFromBytes: (data, fileName) =>
    invoke<ImportOutcome>('library_import_bytes', { data, fileName }),
  listLibraryBooks: () => invoke<LibraryBook[]>('library_list'),
  searchLibraryBooks: (query) =>
    invoke<LibraryBook[]>('library_search', { query }),
  listLibrarySourceRecords: (bookId) =>
    invoke<LibrarySourceRecord[]>('library_source_records', { bookId }),
  searchRemoteLibraryBooks: (query) =>
    invoke<LibraryBook[]>('library_search_remote', { query }),
  searchRemoteLibraryBooksFromSource: (source: RemoteLibrarySource, query) =>
    invoke<LibraryBook[]>('library_search_remote_source', { source, query }),
  acquireRemoteLibraryBook: (id) =>
    invoke<LibraryBook>('library_acquire_remote', { id }),
  linkRemoteToLocalLibraryBook: (remoteId, localId) =>
    invoke<LibraryBook>('library_link_remote_to_local', { remoteId, localId }),
  openLibraryBook: (id) => invoke<OpenedBook>('library_open', { id }),
  touchLibraryLastRead: (id) => invoke('library_touch_last_read', { id }),
  saveAnnotation: (annotation) => invoke('save_annotation', { annotation }),
  listAnnotations: (bookId) => invoke<Annotation[]>('list_annotations', { bookId }),
  deleteAnnotation: (id) => invoke('delete_annotation', { id }),
  saveProgress: (progress) => invoke('save_progress', { progress }),
  getProgress: (bookId) =>
    invoke<ReadingProgress | null>('get_progress', { bookId }),
  resolveFileUrl: (path) => convertFileSrc(path),
  openExternal: (url) => openUrlExternal(url),
  openPathExternal: (path) => openLocalPathExternal(path),
  checkAppUpdate,
  installAppUpdate,
  exportUserDataBackup: async () => {
    let selected: string | string[] | null
    try {
      selected = await open({
        directory: true,
        multiple: false,
        title: '选择备份保存位置',
      })
    } catch (err) {
      throw bridgeError('platformError', '打开备份目录选择器失败', err instanceof Error ? err.message : err)
    }
    if (typeof selected !== 'string') return null
    return invoke<UserDataBackupResult>('export_user_data_backup', {
      destinationParent: selected,
    })
  },
  inspectUserDataBackup: async () => {
    let selected: string | string[] | null
    try {
      selected = await open({
        directory: true,
        multiple: false,
        title: '选择要校验的备份目录',
      })
    } catch (err) {
      throw bridgeError('platformError', '打开备份目录选择器失败', err instanceof Error ? err.message : err)
    }
    if (typeof selected !== 'string') return null
    return invoke<UserDataBackupInspection>('inspect_user_data_backup', {
      backupDir: selected,
    })
  },
  selectPluginPackagePath: async () => {
    const selected = await open({
      multiple: false,
      filters: [{ name: 'LightNovel Reader source plugin', extensions: ['zip'] }],
    })
    return typeof selected === 'string' ? selected : null
  },
  inspectPluginPackage: (path) =>
    invoke<PluginInstallPreview>('plugin_inspect_package', { path }),
  installPluginPackage: (path, confirmUserLegal) =>
    invoke<InstalledPlugin>('plugin_install_package', { path, confirmUserLegal }),
  listInstalledPlugins: () =>
    invoke<InstalledPlugin[]>('plugin_list_installed'),
  setPluginEnabled: (pluginId, enabled) =>
    invoke<InstalledPlugin>('plugin_set_enabled', { pluginId, enabled }),
  uninstallPlugin: (pluginId) =>
    invoke('plugin_uninstall', { pluginId }),
  loadPluginRepositoryIndex: (url) =>
    invoke<PluginRepositoryCatalog>('plugin_load_repository_index', { url }),
  inspectRepositoryPluginPackage: (packageUrl, packageSha256, signature?: PluginPackageSignature) =>
    invoke<PluginInstallPreview>('plugin_inspect_repository_package', { packageUrl, packageSha256, signature }),
  installRepositoryPluginPackage: (packageUrl, packageSha256, signature?: PluginPackageSignature) =>
    invoke<InstalledPlugin>('plugin_install_repository_package', { packageUrl, packageSha256, signature }),
  testPluginFlow: (pluginId, query) =>
    invoke<PluginTestFlowResult>('plugin_test_run', { pluginId, query }),
  listPluginSources: () =>
    invoke<PluginSourceDescriptor[]>('source_list'),
  searchPluginSource: (pluginId, query, page) =>
    invoke<PluginSearchPage>('source_search', { pluginId, query, page }),
  getPluginSourceBook: (pluginId, bookUrl) =>
    invoke<PluginBookDetail>('source_get_book', { pluginId, bookUrl }),
  getPluginSourceChapter: (pluginId, chapterUrl) =>
    invoke<PluginChapterContent>('source_get_chapter', { pluginId, chapterUrl }),
  collectPluginSourceBook: (pluginId, bookUrl) =>
    invoke<LibraryBook>('source_collect', { pluginId, bookUrl }),
  acquirePluginSourceBook: (pluginId, bookUrl) =>
    invoke<LibraryBook>('source_acquire', { pluginId, bookUrl }),
  // ── OPDS v0.6 ──
  opdsAddSource: (name, url) =>
    invoke<OpdsSource>('opds_add_source', { name, url }),
  opdsRemoveSource: (id) =>
    invoke('opds_remove_source', { id }),
  opdsListSources: () =>
    invoke<OpdsSource[]>('opds_list_sources'),
  opdsBrowseFeed: (url) =>
    invoke<OpdsFeed>('opds_browse_feed', { url }),
  opdsSearchFeed: (sourceId, query) =>
    invoke<OpdsFeed>('opds_search_feed', { sourceId, query }),
  opdsIngestEntries: (sourceId, feed) =>
    invoke<LibraryBook[]>('opds_ingest_entries', { sourceId, feed }),
  opdsDownloadEpub: (editionId, acquisitionUrl) =>
    invoke<LibraryBook>('opds_download_epub', { editionId, acquisitionUrl }),
  // sync v1 (Phase 2) — 对接 sync-server
  syncPair: async (code: string) => {
    // code is actually the pairing code from another device
    // server URL stored in localStorage (set by UI)
    const serverUrl = localStorage.getItem('lnr-sync-server-url') || ''
    if (!serverUrl) throw { code: 'invalidArgument' as const, message: '请先设置同步服务器地址' }
    const result = await invoke<{ library_id: string; pairing_code: string; token: string }>(
      'sync_pair_join',
      { serverUrl, pairingCode: code, deviceName: 'desktop' }
    )
    return { libraryId: result.library_id, token: result.token, serverUrl }
  },
  syncStatus: async () => {
    const s = await invoke<{ paired: boolean; lastSyncAt: number | null; pendingChanges: number; libraryId: string | null }>('sync_status')
    return { paired: s.paired, lastSyncAt: s.lastSyncAt, pendingChanges: s.pendingChanges, libraryId: s.libraryId }
  },
  syncNow: async () => {
    // push local changes then pull remote
    await invoke('sync_push', { changes: [] })
    await invoke('sync_pull', { since: null })
  },
  syncUnpair: async () => {
    await invoke('sync_unpair')
    localStorage.removeItem('lnr-sync-server-url')
    localStorage.removeItem('lnr-sync-cred')
  },
}

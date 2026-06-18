// Tauri 桌面壳的 ReaderBridge 实现:协议方法 → Tauri command 的唯一映射点。
// @tauri-apps/* 只允许出现在 src/platform/ 内(scripts/check-arch.mjs 强制)。
import { convertFileSrc, invoke } from '@tauri-apps/api/core'
import { openUrl } from '@tauri-apps/plugin-opener'
import type {
  Annotation,
  BookInfo,
  CalibreBook,
  ImportOutcome,
  LibraryBook,
  LibrarySourceRecord,
  OpdsFeed,
  OpdsSource,
  OpenedBook,
  ReaderBridge,
  ReadingProgress,
  RemoteLibrarySource,
} from './protocol'

export const isTauriRuntime = () =>
  Boolean((window as any).__TAURI_INTERNALS__)

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
  openExternal: (url) => openUrl(url),
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
}

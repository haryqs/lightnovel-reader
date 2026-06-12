// Tauri 桌面壳的 ReaderBridge 实现:协议方法 → Tauri command 的唯一映射点。
// @tauri-apps/* 只允许出现在 src/platform/ 内(scripts/check-arch.mjs 强制)。
import { convertFileSrc, invoke } from '@tauri-apps/api/core'
import type {
  Annotation,
  BookInfo,
  CalibreBook,
  ImportOutcome,
  LibraryBook,
  OpenedBook,
  ReaderBridge,
  ReadingProgress,
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
  listLibraryBooks: () => invoke<LibraryBook[]>('library_list'),
  searchLibraryBooks: (query) =>
    invoke<LibraryBook[]>('library_search', { query }),
  openLibraryBook: (id) => invoke<OpenedBook>('library_open', { id }),
  touchLibraryLastRead: (id) => invoke('library_touch_last_read', { id }),
  saveAnnotation: (annotation) => invoke('save_annotation', { annotation }),
  listAnnotations: (bookId) => invoke<Annotation[]>('list_annotations', { bookId }),
  deleteAnnotation: (id) => invoke('delete_annotation', { id }),
  saveProgress: (progress) => invoke('save_progress', { progress }),
  getProgress: (bookId) =>
    invoke<ReadingProgress | null>('get_progress', { bookId }),
  resolveFileUrl: (path) => convertFileSrc(path),
}

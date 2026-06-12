// platform 适配层:reader-engine 与平台壳之间的唯一边界(方案文档 7 的纪律 1)。
// 引擎代码只允许 import 本目录,不允许直接触碰 @tauri-apps/* 或其他平台 API。
import type { ReaderBridge } from './protocol'
import { isTauriRuntime, tauriBridge } from './tauri'

export * from './protocol'

/** 当前是否运行在带 reading-core 的原生壳里(浏览器直开 vite dev 时为 false)。 */
export const hasNativeBridge = isTauriRuntime

const NO_BRIDGE_HINT = '需要桌面窗口(请运行 npm run tauri dev)'

const unavailable = (method: string): never => {
  throw new Error(`${method} ${NO_BRIDGE_HINT}`)
}

// 纯浏览器环境的兜底实现:除路径透传外全部报错,错误信息可直接展示给用户。
const noBridge: ReaderBridge = {
  openBookFromBytes: async () => unavailable('book.open'),
  openBookFromPath: async () => unavailable('book.openPath'),
  closeBook: async () => {},
  getChapter: async () => unavailable('chapter.get'),
  listCalibreBooks: async () => unavailable('library.listCalibre'),
  importLibraryBook: async () => unavailable('library.import'),
  listLibraryBooks: async () => unavailable('library.list'),
  searchLibraryBooks: async () => unavailable('library.search'),
  openLibraryBook: async () => unavailable('library.open'),
  touchLibraryLastRead: async () => unavailable('library.touchLastRead'),
  saveAnnotation: async () => unavailable('annotation.save'),
  listAnnotations: async () => [],
  deleteAnnotation: async () => unavailable('annotation.delete'),
  saveProgress: async () => {},
  getProgress: async () => null,
  resolveFileUrl: (path) => path,
}

export const bridge: ReaderBridge = isTauriRuntime() ? tauriBridge : noBridge

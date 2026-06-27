/**
 * 主线程 WASM 加载器 —— 加载 reading-core WASM 模块供主线程使用
 * （与 Worker 中的 pagination-worker 使用同一个 wasm 文件，但独立实例）
 */
import initWasm, {
  parse_epub_metadata as wasmParseEpub,
  get_chapter_html as wasmGetChapter,
} from '../worker/reading-core-wasm/reading_core.js'

let _initPromise: Promise<unknown> | null = null

export function initReadingCore(): Promise<unknown> {
  if (!_initPromise) {
    _initPromise = initWasm()
  }
  return _initPromise
}

/** 解析 EPUB 元数据，返回 JSON 字符串 */
export function parseEpubMetadata(data: Uint8Array): string {
  return wasmParseEpub(data)
}

/** 提取并清洗章节 HTML */
export function getChapterHtml(data: Uint8Array, href: string): string {
  return wasmGetChapter(data, href)
}

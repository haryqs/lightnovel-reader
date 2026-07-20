/* tslint:disable */
/* eslint-disable */

/**
 * 提取并清洗章节 HTML（先解析元数据找到 spine，再读取对应文件）
 */
export function get_chapter_html(data: Uint8Array, href: string): string;

/**
 * 把章节 HTML 按 capacity（字符估算）切分成页。
 *
 * `capacity` 应由调用方根据视口宽高/字号/版式参数计算
 * （见 `reader-core.ts::getEstimatedPageCapacity`）。
 */
export function paginate(html: string, capacity: number): string[];

/**
 * 解析 EPUB 元数据，返回 JSON {metadata, toc, spine}
 */
export function parse_epub_metadata(data: Uint8Array): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly paginate: (a: number, b: number, c: number) => [number, number];
    readonly get_chapter_html: (a: number, b: number, c: number, d: number) => [number, number];
    readonly parse_epub_metadata: (a: number, b: number) => [number, number];
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __externref_drop_slice: (a: number, b: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;

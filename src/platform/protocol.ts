// 桥接协议 v1.0-rc.1 —— reader-engine 与 reading-core 之间的全部通信面。
// 任何平台壳(Tauri 桌面 / 将来的 Android、iOS、鸿蒙壳)只要实现 ReaderBridge,
// 引擎即可原样运行。修改本文件 = 修改协议:需同步更新方案文档《8_桥接协议_v0.1》
// 与 reading-core 侧的 serde 结构。

export const PROTOCOL_VERSION = '1.0-rc.1'

// ---- 结构化错误码 ----
// 桥接方法拒绝时，v0.6 起逐步使用结构化 BridgeError；当前 book/chapter/library/annotation/reading/opds/shell
// 已迁移，rejection 值是一个结构化
// BridgeError 对象而非裸字符串，便于引擎据 code 分流（如网络错误可重试、httpStatus 可显示状态）。
// 仍兼容旧的"以字符串消息返回"约定：BridgeError 总带 message，引擎 `e?.message || e` 一律可读。
// 与 reading-core 侧 src-tauri 的 serde 结构（camelCase）一一对应。

/** 结构化错误码枚举。新增 code 必须同步文档 8；Rust command 使用的新 code 还必须同步 Rust 构造器。 */
export type BridgeErrorCode =
  | 'invalidArgument'
  | 'storageError'
  | 'parseError'
  | 'networkError'
  | 'httpStatus'
  | 'notFound'
  | 'forbidden'
  | 'platformError'

export interface BridgeError {
  /** 错误分类，供引擎分流（见 BridgeErrorCode）。 */
  code: BridgeErrorCode
  /** 人类可读消息（中文/英文混合，可直接展示）。 */
  message: string
  /** 可选附加细节（如 HTTP 状态码字符串）。 */
  details?: string
}

/** 运行时判定一个 rejection 是否为结构化 BridgeError（而非裸字符串）。 */
export function isBridgeError(value: unknown): value is BridgeError {
  return (
    typeof value === 'object' &&
    value !== null &&
    typeof (value as BridgeError).code === 'string' &&
    typeof (value as BridgeError).message === 'string'
  )
}

// ---- 数据传输对象(与 reading-core 的 serde 结构一一对应)----

export interface BookInfo {
  metadata: {
    title: string
    author?: string
    language?: string
    description?: string
    series?: string
    seriesIndex?: number
  }
  toc: TocItem[]
  spine: SpineItem[]
}

export interface TocItem {
  label: string
  href: string
  subitems?: TocItem[]
}

export interface SpineItem {
  id: string
  href: string
}

export interface OpenedBook {
  info: BookInfo
  bookId: string
}

export interface CalibreBook {
  title: string
  author: string
  path: string
  coverPath: string
}

export interface LibraryBook {
  id: string
  title: string
  author?: string
  language?: string
  series?: string
  seriesIndex?: number
  description?: string
  /** 库内对象路径。远程 metadata_only 条目无文件 → 缺省。 */
  filePath?: string
  /** 文件字节数。远程条目 → 缺省。 */
  fileSize?: number
  coverPath?: string
  /** 小尺寸缩略图路径（书架优先加载它，回退 coverPath）。 */
  thumbPath?: string
  addedAt: number
  lastReadAt?: number
  // ── v0.5 实体模型可选字段（JOIN asset/edition/volume 回填）。──
  // 本地书库恒有值；旧前端可忽略。供书架系列聚合与远程条目区分。
  /** 系列 id（'series:'名 / 'solo:'bookId）。 */
  seriesId?: string
  /** 卷 id（'vol:'bookId）。 */
  volumeId?: string
  /** 版本 id（'ed:'bookId）。 */
  editionId?: string
  /** 资产可得性（local|remote|missing|cached）：远程条目据此决定能否站内读。 */
  availability?: string
  /** 授权状态（user_owned/public_domain/official_purchase/unknown...）。 */
  rightsStatus?: string
  /** 来源外链：受版权/远程条目点击后跳官方页。本地条目缺省。 */
  remoteUrl?: string
  acquisitionUrl?: string
}

export interface LibrarySourceRecord {
  id: string
  sourceId: string
  sourceName: string
  sourceKind: string
  remoteId?: string
  remoteUrl?: string
  acquisitionUrl?: string
  rightsStatus: string
  availability?: string
  lastCheckedAt?: number
}

export type RemoteLibrarySource = 'anilist' | 'bangumi' | 'aozora' | 'narou'

// ── OPDS v0.6 DTO（与 reading-core connectors::opds serde 结构一一对应）──

export interface OpdsLink {
  rel: string
  href: string
  mimeType?: string
  title?: string
}

export interface OpdsEntry {
  id: string
  title: string
  author?: string
  summary?: string
  links: OpdsLink[]
  /** 封面图片 URL */
  coverUrl?: string
  /** 最佳 EPUB 获取链接 */
  acquisitionUrl?: string
  /** 是否为导航条目（指向子 feed）而非出版物 */
  isNavigation: boolean
}

export interface OpdsFeed {
  title: string
  entries: OpdsEntry[]
  links: OpdsLink[]
}

export interface OpdsSource {
  id: string
  name: string
  baseUrl?: string
  enabled: boolean
}

export interface ImportOutcome {
  book: LibraryBook
  duplicate: boolean
}

export interface Annotation {
  id: string
  bookId: string
  kind: 'highlight' | 'note' | 'bookmark'
  color?: string
  locator: Locator
  note?: string
  createdAt: number
  updatedAt: number
}

export interface Locator {
  chapterHref: string
  anchor: TextAnchor
}

export interface ReadingProgress {
  bookId: string
  /** spine 规范形式的章节 href */
  chapterHref: string
  /** 章内进度 0..1(页索引比例,字号/版式变化下近似稳定) */
  chapterProgress: number
  /** 全书进度 0..1(展示用) */
  percentage: number
  updatedAt: number
}

export interface TextAnchor {
  start: number       // 章内字符偏移
  end: number
  exact: string       // 精确文本(用于重定位时找位置)
  prefix: string      // 前 20 字符(帮助消歧义)
  suffix: string      // 后 20 字符
}

// ── v0.7 插件安装 DTO（只读包信息与安装元数据，不执行插件代码）──

export type PluginPermission = 'http' | 'kv'
export type PluginCapability = 'browse' | 'resolveUrl' | 'fetchMetadata' | 'acquire'
export type PluginLegalKind = 'public-domain' | 'open-license' | 'official-free' | 'user-declared'

export interface PluginLegal {
  kind: PluginLegalKind
  note?: string
}

export interface PluginManifest {
  apiVersion: string
  id: string
  name: string
  version: string
  description?: string
  author?: string
  language?: string
  entry: string
  domains: string[]
  permissions: PluginPermission[]
  capabilities: PluginCapability[]
  legal: PluginLegal
}

export interface PluginValidation {
  officialRepositoryEligible: boolean
  requiresUserLegalConfirmation: boolean
  warnings: string[]
}

export interface PluginInstallPreview {
  manifest: PluginManifest
  validation: PluginValidation
  entrySize: number
}

export interface InstalledPlugin extends PluginInstallPreview {
  installedAt: number
}

// ---- 桥接接口:每个方法对应协议里的一条消息 ----

export interface ReaderBridge {
  /** book.open — 从内存字节打开 EPUB,返回书籍结构 */
  openBookFromBytes(data: Uint8Array): Promise<BookInfo>
  /** book.openPath — 按文件路径打开(书库进入),bookId 由 core 按内容哈希计算 */
  openBookFromPath(path: string): Promise<OpenedBook>
  /** book.close — 释放当前书的内存 */
  closeBook(): Promise<void>
  /** chapter.get — 返回清洗后的章节 HTML */
  getChapter(href: string): Promise<string>
  /** library.listCalibre — 列出 Calibre 书库中的全部 EPUB */
  listCalibreBooks(library: string): Promise<CalibreBook[]>
  /** library.import — 导入 EPUB 到自有书库对象仓库 */
  importLibraryBook(path: string): Promise<ImportOutcome>
  /** library.importBytes — 从文件选择器字节导入 EPUB 到自有书库对象仓库 */
  importLibraryBookFromBytes(data: Uint8Array, fileName?: string): Promise<ImportOutcome>
  /** library.list — 列出自有书库 */
  listLibraryBooks(): Promise<LibraryBook[]>
  /** library.search — 搜索自有书库 */
  searchLibraryBooks(query: string): Promise<LibraryBook[]>
  /** library.listSourceRecords - list remote source records attached to a library entry */
  listLibrarySourceRecords(bookId: string): Promise<LibrarySourceRecord[]>
  /** library.searchRemote — 在线元数据搜索（AniList），落库为远程条目并返回 */
  searchRemoteLibraryBooks(query: string): Promise<LibraryBook[]>
  /** library.searchRemoteSource — 指定在线来源搜索（anilist|bangumi|aozora|narou），按需缓存来源目录 */
  searchRemoteLibraryBooksFromSource(source: RemoteLibrarySource, query: string): Promise<LibraryBook[]>
  /** library.acquireRemote — 获取公共版权远程条目的正文并转为本地可读资产 */
  acquireRemoteLibraryBook(id: string): Promise<LibraryBook>
  /** library.linkRemoteToLocal — 人工关联远程元数据条目到本地可读资产 */
  linkRemoteToLocalLibraryBook(remoteId: string, localId: string): Promise<LibraryBook>
  /** library.open — 按自有书库 id 打开书籍 */
  openLibraryBook(id: string): Promise<OpenedBook>
  /** library.touchLastRead — 更新最近阅读时间 */
  touchLibraryLastRead(id: string): Promise<void>
  /** annotation.save — upsert 一条标注 */
  saveAnnotation(annotation: Annotation): Promise<void>
  /** annotation.list — 按 bookId 列出标注 */
  listAnnotations(bookId: string): Promise<Annotation[]>
  /** annotation.delete */
  deleteAnnotation(id: string): Promise<void>
  /** reading.saveProgress — upsert 一本书的阅读位置 */
  saveProgress(progress: ReadingProgress): Promise<void>
  /** reading.getProgress — 无记录返回 null */
  getProgress(bookId: string): Promise<ReadingProgress | null>
  /** resource.url — 把本地文件路径转成当前 WebView 可加载的 URL(同步) */
  resolveFileUrl(path: string): string
  /** shell.openExternal — 用系统默认浏览器打开外链(远程条目跳官方页) */
  openExternal(url: string): Promise<void>
  /** shell.openPathExternal - open a local readable asset with the system default app */
  openPathExternal(path: string): Promise<void>
  /** plugin.selectPackagePath — 选择源插件 zip 安装包，返回本地路径或 null */
  selectPluginPackagePath(): Promise<string | null>
  /** plugin.inspectPackage — 读取 zip manifest/入口并返回安装前确认信息，不执行插件代码 */
  inspectPluginPackage(path: string): Promise<PluginInstallPreview>
  /** plugin.installPackage — 用户确认后写入本地插件目录，不执行插件代码 */
  installPluginPackage(path: string, confirmUserLegal: boolean): Promise<InstalledPlugin>
  /** plugin.listInstalled — 列出已安装源插件元数据 */
  listInstalledPlugins(): Promise<InstalledPlugin[]>
  // ── OPDS v0.6 ──
  /** opds.addSource — 添加一个 OPDS 书源 */
  opdsAddSource(name: string, url: string): Promise<OpdsSource>
  /** opds.removeSource — 移除一个 OPDS 书源 */
  opdsRemoveSource(id: string): Promise<void>
  /** opds.listSources — 列出所有 OPDS 书源 */
  opdsListSources(): Promise<OpdsSource[]>
  /** opds.browseFeed — 抓取一个 OPDS feed 并解析（不做落库） */
  opdsBrowseFeed(url: string): Promise<OpdsFeed>
  /** opds.searchFeed — 在指定 OPDS 书源中搜索 */
  opdsSearchFeed(sourceId: string, query: string): Promise<OpdsFeed>
  /** opds.ingestEntries — 把 feed 条目落库为远程书库条目 */
  opdsIngestEntries(sourceId: string, feed: OpdsFeed): Promise<LibraryBook[]>
  /** opds.downloadEpub — 下载 OPDS open_license EPUB 并转为本地可读资产 */
  opdsDownloadEpub(editionId: string, acquisitionUrl?: string): Promise<LibraryBook>
}

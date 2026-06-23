// 源插件契约 v0.1 —— 一个源插件 = default 导出一个实现 SourcePlugin 的对象。
//
// 设计目标:写一个源 = 填三个函数(search / getBook / getChapter)。
// 站点专属的 CSS 选择器规则由插件作者维护,这是“过滤准确”的真正来源。

export {}

declare global {
  /** 插件入口:`export default { search, getBook, getChapter }` */
  interface SourcePlugin {
    /** 在该源中搜索。query 为用户输入;page 从 1 开始。 */
    search(query: string, page: number): Promise<SearchPage>
    /** 拉取一本书/一个系列的详情与章节列表。bookUrl 来自 SearchResult.url。 */
    getBook(bookUrl: string): Promise<BookDetail>
    /** 拉取一章正文。chapterUrl 来自 BookDetail.chapters[].url。 */
    getChapter(chapterUrl: string): Promise<ChapterContent>
    /** Optional: category/ranking browsing. Requires manifest.capabilities: browse. */
    browse?(path: string, page: number): Promise<SearchPage>
    /** Optional: resolve a pasted source URL. Requires manifest.capabilities: resolveUrl. */
    resolveUrl?(url: string): Promise<SearchResult | null>
    /** Optional: metadata-only lookup. Requires manifest.capabilities: fetchMetadata. */
    fetchMetadata?(remoteId: string): Promise<BookDetail | null>
    /**
     * Optional: propose acquisition. Requires manifest.capabilities: acquire.
     * The host still enforces domain, rights and ToS gates. In v0.7 policy,
     * download/cacheForReading is only allowed for public_domain/open_license;
     * official_free stays metadata/external-link only until a source-specific ToS/rate-limit gate exists.
     */
    acquire?(remoteId: string, mode: AcquireMode): Promise<AcquireProposal>
  }

  interface SearchPage {
    results: SearchResult[]
    /** 是否还有下一页。 */
    hasMore: boolean
  }

  interface SearchResult {
    /** 该书在源站的规范 URL,同时作为后续 getBook 的入参与去重键。 */
    url: string
    title: string
    author?: string
    coverUrl?: string
    /** 一句话简介,列表页展示用。 */
    summary?: string
  }

  interface BookDetail {
    url: string
    title: string
    author?: string
    coverUrl?: string
    description?: string
    /** 章节按阅读顺序排列。 */
    chapters: ChapterRef[]
  }

  interface ChapterRef {
    url: string
    title: string
    /** 卷/分组名,可选(轻小说常见“第一卷”分组)。 */
    group?: string
  }

  interface ChapterContent {
    title: string
    /**
     * 章节正文 HTML 片段。只允许文本类标签(p/ruby/rt/em/strong/br/img 等),
     * 宿主入库前还会过 reading-core 的 HTML 清洗,脚本与样式一律剥除。
     */
    html: string
  }

  type AcquireMode = 'metadataOnly' | 'download' | 'cacheForReading'

  interface AcquireProposal {
    /** Candidate acquisition URL. Host must still enforce domain, rights and ToS gates. */
    url: string
    /** Plugin-declared rights status; never the final host decision by itself. */
    rightsStatus: 'public_domain' | 'open_license' | 'official_free' | 'unknown'
    /** Optional content type hint, such as application/epub+zip or text/html. */
    mimeType?: string
    /** User-facing source note or ToS warning. */
    note?: string
  }
}

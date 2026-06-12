// 插件宿主 API v0.1 —— 源插件能触达的全部世界。
//
// 运行环境:引擎无关的纯 JS 子集(ES2020)。没有 DOM、没有 Node API、没有
// fetch/XMLHttpRequest、没有文件系统、没有动态加载代码。所有真实能力由
// reading-core 的 Rust 侧实现,经 `host` 注入;桌面/Android/鸿蒙跑 QuickJS,
// iOS 跑系统 JavaScriptCore(App Store 2.5.2),插件代码两边一字不改。
//
// 网络访问仅限 manifest.domains 声明的域名;越界请求被宿主直接拒绝。
//
// 宿主额外注入的标准全局(两个引擎行为一致,由宿主保证):URL、TextDecoder。
// 除此之外不要假设任何浏览器/Node 全局存在。

export {}

declare global {
  const host: HostApi

  interface HostApi {
    http: HostHttp
    html: HostHtml
    kv: HostKv
    log: HostLog
  }

  interface HostHttp {
    /** GET 请求。url 的域名必须在 manifest.domains 内,否则 reject。 */
    get(url: string, opts?: HttpOptions): Promise<HttpResponse>
  }

  interface HttpOptions {
    /** 附加请求头。User-Agent/Referer 等由宿主统一控制,这里设置无效。 */
    headers?: Record<string, string>
    /** 超时毫秒,默认 15000,上限 60000。 */
    timeoutMs?: number
  }

  interface HttpResponse {
    status: number
    headers: Record<string, string>
    /** 响应体文本。宿主按 Content-Type/BOM 自动解码(含 Shift_JIS 等)。 */
    text(): string
  }

  interface HostHtml {
    /** 把 HTML 文本解析为可查询的文档(宿主侧 Rust 解析,非浏览器 DOM)。 */
    parse(htmlText: string): HtmlDoc
  }

  interface HtmlDoc {
    /** CSS 选择器,返回全部命中元素。 */
    select(selector: string): HtmlElement[]
    selectFirst(selector: string): HtmlElement | null
  }

  interface HtmlElement {
    /** 元素内纯文本(已合并空白)。 */
    readonly text: string
    /** 元素内层 HTML。 */
    readonly innerHtml: string
    attr(name: string): string | null
    select(selector: string): HtmlElement[]
    selectFirst(selector: string): HtmlElement | null
  }

  interface HostKv {
    /** 插件私有键值存储(按插件 id 隔离),用于缓存 token、目录页等。 */
    get(key: string): Promise<string | null>
    set(key: string, value: string): Promise<void>
    delete(key: string): Promise<void>
  }

  interface HostLog {
    info(...args: unknown[]): void
    warn(...args: unknown[]): void
    error(...args: unknown[]): void
  }
}

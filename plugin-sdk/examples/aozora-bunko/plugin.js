// 青空文库源插件 —— 官方示例骨架。
// 演示源插件三件套:search / getBook / getChapter。
// 注意:这是契约演示用骨架,CSS 选择器与 URL 模板尚未对站校准,
// 接入真实运行时(v0.7)前需用实际页面验证。

const BASE = 'https://www.aozora.gr.jp'

export default {
  async search(query, page) {
    // 青空文库无服务端搜索 API,实际实现应抓取作品一覧索引页并本地过滤;
    // 索引页很大,正式实现时用 host.kv 缓存。
    const resp = await host.http.get(`${BASE}/index_pages/person_all.html`)
    const doc = host.html.parse(resp.text())
    const results = doc
      .select('li a')
      .filter((a) => a.text.includes(query))
      .slice((page - 1) * 20, page * 20)
      .map((a) => ({
        url: new URL(a.attr('href') ?? '', BASE).toString(),
        title: a.text,
      }))
    return { results, hasMore: results.length === 20 }
  },

  async getBook(bookUrl) {
    const resp = await host.http.get(bookUrl)
    const doc = host.html.parse(resp.text())
    const title = doc.selectFirst('h1, .title')?.text ?? bookUrl
    const author = doc.selectFirst('h2, .author')?.text ?? undefined
    // 青空文库一部作品通常是单个 XHTML 整页;把它作为唯一“章节”。
    const reading = doc.selectFirst('a[href*="files/"]')
    const chapters = reading
      ? [{ url: new URL(reading.attr('href') ?? '', bookUrl).toString(), title: '本文' }]
      : []
    return { url: bookUrl, title, author, chapters }
  },

  async getChapter(chapterUrl) {
    const resp = await host.http.get(chapterUrl)
    const doc = host.html.parse(resp.text())
    const main = doc.selectFirst('.main_text, body')
    return {
      title: doc.selectFirst('.title, h1')?.text ?? '',
      // 青空文库正文自带 ruby 标签(振假名),原样保留,交给宿主清洗器。
      html: main?.innerHtml ?? '',
    }
  },
}

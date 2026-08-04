// Project Gutenberg 公共领域书籍来源。

const BASE = 'https://www.gutenberg.org'

export default {
  async search(query, page) {
    const start = (page - 1) * 25 + 1
    const response = await host.http.get(
      `${BASE}/ebooks/search.opds/?query=${encodeURIComponent(query)}&start_index=${start}`,
    )
    const feed = response.text()
    const doc = host.html.parse(feed)
    const entries = doc.select('entry')
    host.log.info(
      '[gutenberg] search',
      response.status,
      response.headers['content-type'] || '',
      feed.length,
      `entries=${entries.length}`,
      `html=${doc.select('html').length}`,
    )
    const results = []
    const seen = {}

    for (const entry of entries) {
      const title = entry.selectFirst('title')?.text || ''
      const id = entry.selectFirst('id')?.text || ''
      const idMatch = id.match(/\/ebooks\/(\d+)\.opds(?:[?#]|$)/)
      if (!idMatch || !title) continue
      const url = `${BASE}/ebooks/${idMatch[1]}`
      if (seen[url]) continue
      seen[url] = true
      results.push({
        url,
        title,
        author: entry.selectFirst('name')?.text || undefined,
      })
    }
    let hasMore = false
    for (const link of doc.select('link')) {
      if ((link.attr('rel') || '') === 'next') {
        hasMore = true
        break
      }
    }
    return { results, hasMore }
  },

  async getBook(bookUrl) {
    const response = await host.http.get(bookUrl)
    const doc = host.html.parse(response.text())
    const title = doc.selectFirst('h1')?.text || bookUrl
    let author
    for (const row of doc.select('tr')) {
      const heading = row.selectFirst('th')?.text || ''
      if (heading.includes('Author')) author = row.selectFirst('td')?.text || undefined
    }

    const chapters = []
    for (const link of doc.select('a')) {
      const href = link.attr('href') || ''
      if (!/\/(?:cache\/epub|files)\//.test(href) || !/\.html?($|[?#])/i.test(href)) continue
      chapters.push({
        url: new URL(href, bookUrl).toString(),
        title: link.text || title,
      })
      break
    }
    return { url: bookUrl, title, author, chapters }
  },

  async getChapter(chapterUrl) {
    const response = await host.http.get(chapterUrl)
    const doc = host.html.parse(response.text())
    const body = doc.selectFirst('body')
    return {
      title: doc.selectFirst('h1, h2')?.text || chapterUrl,
      html: body?.innerHtml || '',
    }
  },

  async acquire(bookUrl, mode) {
    const response = await host.http.get(bookUrl)
    const doc = host.html.parse(response.text())
    for (const link of doc.select('a')) {
      const href = link.attr('href') || ''
      if (!/\.epub3?\.images($|[?#])/i.test(href)) continue
      const url = new URL(href, bookUrl).toString()
      host.log.info('[gutenberg] acquire', mode, url)
      return {
        url,
        rightsStatus: 'public_domain',
        mimeType: 'application/epub+zip',
        note: 'Project Gutenberg public-domain EPUB',
      }
    }
    throw new Error('当前 Gutenberg 书籍页没有可获取的 EPUB 下载链接')
  },
}

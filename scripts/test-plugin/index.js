/**
 * 离线 QuickJS 测试插件：验证 SDK 三个必选方法、host.kv 与 host.log。
 */

export default {
  async search(query, page) {
    host.log.info('[test-plugin] search', query, page)
    const cached = await host.kv.get('search_count')
    const count = cached ? Number(cached) + 1 : 1
    await host.kv.set('search_count', String(count))

    return {
      results: [
        {
          url: 'https://example.com/books/book-1',
          title: `测试小说：${query}`,
          author: 'QuickJS 运行时',
          summary: `离线测试书；累计搜索 ${count} 次`,
        },
        {
          url: 'https://example.com/books/book-2',
          title: '第二本测试书',
          author: 'Plugin Test',
        },
      ],
      hasMore: false,
    }
  },

  async getBook(bookUrl) {
    host.log.info('[test-plugin] getBook', bookUrl)
    return {
      url: bookUrl,
      title: '测试小说详情',
      author: 'QuickJS 运行时',
      description: '通过 QuickJS 插件运行时获取的离线书籍详情。',
      chapters: [
        { url: `${bookUrl}/chapters/1`, title: '第一章 开始' },
        { url: `${bookUrl}/chapters/2`, title: '第二章 中间' },
        { url: `${bookUrl}/chapters/3`, title: '第三章 结束' },
      ],
    }
  },

  async getChapter(chapterUrl) {
    host.log.info('[test-plugin] getChapter', chapterUrl)
    return {
      title: '第一章 开始',
      html: `<h1>第一章 开始</h1><p>章节 URL：${chapterUrl}</p><p>如果看到这段内容，说明完整插件流程工作正常。</p>`,
    }
  },
}

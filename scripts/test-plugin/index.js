/**
 * Hello World 测试插件 —— 验证 QuickJS 运行时端到端流程。
 *
 * 不依赖外部网络，所有数据硬编码。
 * 可用于测试：search、getBook、getChapter、host.kv、host.log。
 */

var plugin = {};

plugin.search = function (args) {
  var query = (args && args.query) || '';
  host.log.info('[test-plugin] search called with: ' + query);

  // 测试 host.kv 读写
  var cached = host.kv.get('search_count');
  var count = cached ? parseInt(cached) + 1 : 1;
  host.kv.set('search_count', String(count));

  return [
    {
      id: 'book-1',
      title: '测试小说：' + query,
      author: 'QuickJS 运行时',
      coverUrl: null,
      summary: '这是一本由测试插件生成的小说。搜索次数：' + count,
      language: 'zh',
    },
    {
      id: 'book-2',
      title: '第二本测试书',
      author: 'Plugin Test',
      coverUrl: null,
      summary: '第二本硬编码测试书。',
      language: 'zh',
    },
  ];
};

plugin.getBook = function (args) {
  var id = (args && args.id) || 'book-1';
  host.log.info('[test-plugin] getBook: ' + id);

  return {
    id: id,
    title: '测试小说详情',
    author: 'QuickJS 运行时',
    coverUrl: null,
    summary: '这是书籍 ' + id + ' 的详情。通过 QuickJS 插件运行时获取。',
    language: 'zh',
    chapters: [
      { id: 'ch1', title: '第一章 开始' },
      { id: 'ch2', title: '第二章 中间' },
      { id: 'ch3', title: '第三章 结束' },
    ],
  };
};

plugin.getChapter = function (args) {
  var bookId = (args && args.bookId) || 'book-1';
  var chapterId = (args && args.chapterId) || 'ch1';
  host.log.info('[test-plugin] getChapter: ' + bookId + '/' + chapterId);

  return '<h1>' + chapterId + '</h1><p>这是测试插件生成的章节内容。</p><p>书籍 ID：' + bookId + '</p><p>章节 ID：' + chapterId + '</p><p>本章由 QuickJS 运行时动态生成，不依赖外部网络。</p><p>如果你看到这段文字，说明 QuickJS 插件运行时工作正常！</p>';
};

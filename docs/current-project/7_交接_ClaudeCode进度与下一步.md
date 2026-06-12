# 交接：Claude Code → opencode（reader 开发）

> 日期 2026-06-07。本仓库 reader 代码先前由 Claude Code 与 opencode **并行编辑**，
> 现统一交给 **opencode 主驾驶**。本文件交接：已完成、已验证状态、待清理与下一步。
> 凡涉及 `reader/` 代码，Claude Code 已停手，避免再并发冲突。

---

## 一、已完成并提交（截至 9c6a0cf）

| 提交 | 内容 | 谁 |
|---|---|---|
| `9c6a0cf` | Windows 图片协议 URL 修复 + 临时 `open_book_path` | CC 协助 |
| `f5c305a` | TOC 显示文件名修复 + 封面/图片 | opencode |
| `18c5395` | v0.4 标注系统（高亮/批注/侧栏/SQLite） | opencode |
| `65e2e02` | 速度三连击（拆元数据解析/去 base64/双缓冲预载） | opencode |
| `a31b74f` | v0.3 HTML 深度清洗（去 font/行高/注入排版基线） | opencode |
| 更早 `fe1f8f0` | 大书"未响应"卡死修复（async 开书 + 预载降为 1） | CC |
| 更早 `61e02af` | 翻页改为无滚动条整屏翻页（`overflow:hidden`+整页吸附） | CC |
| 更早 `b41bdbd` | 目录 NCX 自闭合标签、正文链接拦截、SVG 空白页、跨章 href、键盘分工、进度条焦点 | CC |

## 二、已用 computer use **实测验证**的关键点

1. **大书不再卡死**：之前"未响应"的 4MB《年代四部曲》现在 **43ms 打开**，翻章正常。
2. **翻页是整屏翻页**：无滚动条，←/→ 整屏替换（实测 `scrollTop` 每次精确跳一个 `clientHeight`）。
   - 关键发现：Gutenberg 式 EPUB 会把整本塞进一个 67000+px 的巨章，这是先前"翻页像只滚动一点"的根因。
3. **封面/图片显示**：`reader-img` 协议在 **Windows 上必须用 `http://reader-img.localhost/<path>`**，
   不是 `reader-img://localhost/...`（后者在 WebView2 不解析 → 破图）。已在 `html_sanitizer.rs::img_html` 用
   `#[cfg(windows)]` 处理。实测《年代四部曲》封面正常渲染。

## 三、待清理 / 已知问题（请 opencode 优先处理）

1. **`npm run build` 当前会失败**——TS6133 未使用变量（`tsc` 严格模式）：
   - `src/annotations.ts:39` 形参 `chapterHref` 未用
   - `src/reader-core.ts:3` 导入 `HIGHLIGHT_COLORS` 未用
   - `src/reader-core.ts:205` 字段 `cachedSpineIdx` 未用
   （`pnpm dev`/`tauri dev` 用 esbuild 不报，但打包会挂。先清掉。）
2. **临时调试代码要删**（在 `9c6a0cf` 里混入了）：
   - `lib.rs`：`open_book_path` 命令（按路径开书，仅为绕过文件对话框调试用）
   - `lib.rs`：图片协议处理器里的 `eprintln!("[reader-img] 请求: ...")`
3. **重复的标注实现要二选一**：
   - Claude Code 建的地基：`src/annotation/locator.ts`、`src/annotation/storage.ts`、`src-tauri/src/storage.rs`
   - opencode 的现行实现：`src/annotations.ts`（v0.4 已用这个）
   - 现行是 opencode 的；**建议删掉 `src/annotation/` 那套**（若其 Rust storage.rs 未被现行链路使用也一并清），避免两套并存。

## 四、下一步建议（功能/打磨）

- **图片健壮性**：找不到的图返回 404 → 仍是破图标。考虑 miss 时返回 1×1 透明或隐藏，避免难看的破图。
- **跨平台 URL**：`img_html` 已对 Windows/Android 用 `http://<scheme>.localhost/`；macOS/Linux 走 `reader-img://localhost/`。将来出 mac/linux 包时验证一次。
- **大书巨章体验**：一个 67000px 的巨章 = 107 个翻页页。进度条/页码语义偏"章级"，可加"本章 X/Y 页"。
- **OCR（远期，非同质化卖点）**：用户的核心书是扫描版 PDF（无文字层）。v2 PDF 阶段，Rust 端 OCR → 让扫描经典可搜索/可标注，是真差异化方向。PDF 渲染实测 MuPDF（`mupdf` crate，AGPL 对口）打开 70MB 扫描件 5–30ms、翻页 13ms，远胜 PDF.js。
- **标注锚模型**：若现行 `annotations.ts` 用的是 CFI 风格锚，注意正文是 Rust **清洗后**的 HTML，CFI 会脆；
  Claude Code 地基里的"字符位置主锚 + 文本引用兜底（TextQuote）"更稳、且能跨 TXT/PDF 统一，可参考 `src/annotation/locator.ts` 的 `relocate`。

## 五、构建环境坑（务必知道）

- 本机全局 cargo 镜像 `ustc` **SSL 挂了**，直接 `cargo`/`tauri dev` 会卡在 `Updating ustc index`。
  绕过：构建时设环境变量 `CARGO_SOURCE_ustc_REGISTRY="sparse+https://rsproxy.cn/index/"`（rsproxy/tuna 实测可达）。
  根治：改全局 `~/.cargo/config.toml` 的镜像。
- dev 端口是 **3000**；换二进制时若旧 `reader.exe` 还在跑会报"拒绝访问 os error 5"，先杀进程。

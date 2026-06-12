# 阅读器（Reader）

轻量 epub 阅读器，基于 **Tauri v2 + 原生 TypeScript + Vite**。
打包后体积小、启动快，正文渲染在 WebView 中。

## v1 已实现功能

- 打开本地 epub 文件
- 分页阅读（翻书式，非长卷滚动）
- 目录（TOC）侧栏，点击跳转，自动高亮当前章节
- 三套护眼主题：日间 / 护眼（米色）/ 夜间——主题会注入正文 iframe
- 字号增减（A- / A+）
- 阅读进度：底部百分比 + 可拖动进度条
- 进度持久化：同一本书重开自动回到上次位置（localStorage，按 文件名+大小 标识）
- 翻页方式：底部按钮 / 左右点击热区 / 键盘 ← → 方向键

## 目录结构

```
reader/
├── index.html              界面骨架
├── src/
│   ├── main.ts             UI 交互层（按钮/热区/进度条 → Reader）
│   ├── reader.ts           阅读器核心（封装 epubjs：加载/翻页/主题/进度）
│   ├── themes.ts           三套主题 + 正文排版（注入 epub iframe）
│   └── styles.css          界面样式 + 主题 CSS 变量
└── src-tauri/              Tauri 后端（Rust）
```

> 架构约定：UI 只跟 `reader.ts` 的 `Reader` 类打交道，不直接碰 epubjs。
> 这样以后换渲染引擎（如 foliate-js）或加 PDF 支持时，改动只集中在核心层。

## 开发与运行

### 仅前端（不需要 Rust，浏览器里调试）

```bash
npm install
npm run dev      # 启动 Vite，浏览器打开提示的地址
```

> 注：本机 Windows 保留了 1420 端口，dev 端口已改为 **3000**。

### 完整桌面 App（需要 Rust 工具链）

先装好 Rust + C++ Build Tools（见仓库根 CLAUDE.md「开发前置条件」），然后：

```bash
npm run tauri dev      # 开发模式，带热重载
npm run tauri build    # 打包成原生安装包
```

## 已知事项 / 后续计划

- epubjs 依赖的旧版 `@xmldom/xmldom` 有安全告警；仅打开本地可信文件，风险低。
  后续可迁移到 **foliate-js**（更现代、无依赖、原生支持 epub + PDF）。
- 下一步：PDF 支持、高亮划线与批注、批注导出（对接 Obsidian）、插件系统。

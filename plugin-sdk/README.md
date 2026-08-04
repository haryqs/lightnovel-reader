# 源插件 SDK(契约 v0.1)

> 桌面端 QuickJS 运行时已接入 v0.7。本目录是 manifest、宿主 API、插件接口与
> 示例插件的代码契约权威入口。
> 背景与设计推导见 `workspace/reader-resource-library-plan/9_插件契约_v0.1.md`。

## 一个源插件 = 一个目录

```text
my-source/
├── manifest.json   # 声明:id、域名白名单、权限、授权性质(见 manifest.schema.json)
└── plugin.js       # 入口:export default { search, getBook, getChapter }
```

分发时打成 zip。zip 可以直接包含 `manifest.json + plugin.js`,也可以外面包一层同名目录;
宿主只允许一个 `manifest.json`,入口脚本必须与 manifest 同目录,且必须是单个 `.js` 文件名。
`reading-core::plugin_package` 会在安装前读取 zip、校验 manifest、确认入口脚本存在,但不会执行插件代码。
桌面端安装 UI 会展示域名、权限、能力和授权声明；`user-declared` 插件必须由用户显式确认后才会写入本地插件目录。

## 三个文件

| 文件 | 作用 |
|---|---|
| `manifest.schema.json` | manifest 的 JSON Schema,安装时逐项展示给用户 |
| `repository.schema.json` | 官方白名单插件仓库索引的 JSON Schema,声明 HTTPS zip、SHA-256 与可选签名元数据 |
| `host-api.d.ts` | 插件能用的全部宿主能力:`host.http` / `host.html` / `host.kv` / `host.log` |
| `source-plugin.d.ts` | 插件必须实现的接口:`search` / `getBook` / `getChapter` |

`manifest.capabilities` 只声明可选能力:`browse`、`resolveUrl`、`fetchMetadata`、`acquire`。
宿主会根据 manifest、授权性质和 ToS 门控决定是否显示/放行这些能力；插件返回值不能自行决定可缓存正文。
`reading-core::plugin_host` 已提供运行前策略层：停用插件不运行，未声明 capability 的可选方法不运行，
`host.http` 只允许 manifest 域名并会忽略 User-Agent/Referer/Cookie/Authorization 等保留头，
`host.kv` 有插件私有桶与尺寸限制。桌面壳已经把这些策略门接入 QuickJS：每次方法调用使用
一次性 Runtime/Context，支持 SDK 的 `export default`、Promise、`host.http/html/kv/log`，并在返回前按 DTO 校验和清洗章节 HTML。
插件返回的书籍、章节和封面 URL 也必须属于 manifest 精确域名；单页搜索结果、章节数和文本长度有硬上限。

## 试跑与示例

- `scripts/test-plugin/test-plugin-hello.zip`：无网络的硬编码冒烟插件，用于验证安装和完整
  `search → getBook → getChapter` 调用。修改源码后运行 `node scripts/package-test-plugin.mjs` 重新打包。
- `examples/gutenberg/gutenberg.zip`：首个正式官方来源，提供 Project Gutenberg 公共领域书籍
  搜索、正文预览与 EPUB 获取。搜索使用官方 OPDS Atom feed；离线夹具固定解析契约，
  人工/忽略的联网 E2E 用于发现源站接口或网络环境变化。
- 桌面端已安装插件右侧的“测试”按钮会依次调用三个必选方法，并展示结构化结果。
- 启用插件还会出现在书库“在线来源”下拉框中。正式流程支持分页搜索、书籍/章节详情和纯文本正文预览；
  搜索不会自动入库，用户点“收藏来源”后宿主会重新执行 `getBook`，只把元数据与源站外链写成远程来源记录。
- `public-domain/open-license` 插件声明 `acquire` 后可返回同域 `application/epub+zip` 提案；来源 UI 会显示
  “获取并阅读”，宿主复核授权/域名、下载并验证 EPUB 后直接写入本地对象仓库。`source.acquire` 不改变收藏语义。

## 安全模型(写插件前必读)

- 插件跑在沙箱 JS 引擎里(桌面/Android/鸿蒙:QuickJS;iOS:JavaScriptCore)。
- 没有 DOM、Node、fetch、文件系统;能力只来自 `host`。
- `host.http` 只放行 `manifest.domains` 列出的域名。
- SDK 返回的 `url/coverUrl/chapterUrl` 同样只允许 `manifest.domains` 精确域名，不能返回 `javascript:`、`file:` 或未声明站点。
- 桌面 HTTP 执行器不自动跟随重定向，会拒绝本机/内网目标，并对响应体设置 8 MiB 上限。
- `host.html` 输入与插件返回 JSON 上限均为 8 MiB；单条日志上限为 4 KiB。
- ES2020 纯 JS 子集;不要依赖任何浏览器/Node 全局(`URL` 等由宿主按 API 版本提供)。

## 合规边界

官方插件仓库只收 `legal.kind` 为 `public-domain` / `open-license` / `official-free`
的源,且 `official-free` 必须遵守源站 ToS(含抓取频率)。内核不内置任何源。
`user-declared` 插件只能用户自装,安装时必须做明示确认,UI 上也必须与官方插件区分。
v0.7 第一版下载/缓存正文只放行 `public_domain` 与 `open_license`；
`official_free` 即使已具备通用 ToS 确认和每域限速，仍只做元数据、临时预览与官方外链；单源正文缓存需另行审核。

## 官方仓库索引

官方仓库索引采用 `repository.schema.json`，每个条目包含完整 manifest、`packageUrl`、
`packageSha256`、可选 `packageSize/sourceUrl/signature`。`reading-core::plugin_repository`
会校验索引版本、官方仓库资格、重复 id、HTTPS URL、SHA-256 形状、包大小和签名元数据形状。
签名覆盖插件 zip 的原始字节：`keyId` 必须属于桌面壳编译内 keyring，预览和安装都会重新下载，
先核对 SHA-256，再以 Ed25519 验签，最后进入 `plugin_package` / `plugin_store` 校验与用户确认。
未配置发布公钥时，unsigned 条目只以明确的人工白名单警告模式加载；任何伪称签名、未知 keyId 或坏签名都会拒绝。

仓库维护者先用
`npm.cmd run prepare:plugin-repository-release -- --package <zip> --base-url <GitHub Release 资产目录>
--out-dir <仓库外暂存目录> --source-url <源码页>` 从 zip 内真实 manifest 生成 unsigned 索引、复制包并计算
SHA-256/大小；已有输出默认拒绝覆盖，复跑时须显式 `--force`。再用
`npm.cmd run sign:plugin-repository -- --repository <repository.unsigned.json> --package-dir <zip目录>
--private-key <PKCS#8 PEM> --key-id <id> --expected-public-key-base64 <编译内公钥> --out <repository.json>`
签署索引内每个 zip。签名脚本会再次核对 SHA-256/大小，并在写出前确认私钥与编译内公钥匹配；
私钥只从外部路径读取且不得进入仓库；输出的 `publicKeyBase64` 才可加入 `src-tauri/src/plugin_trust.rs`。
上传前再运行
`npm.cmd run verify:plugin-repository-release -- --repository <repository.json> --package-dir <zip目录>
--public-key-base64 <编译内公钥> --key-id <id>`，只用公钥独立复核所有包的哈希、大小、keyId 与签名。
可运行 `npm.cmd run smoke:plugin-repository-signature` 做无需公网/GUI 的发布链回归：脚本生成临时 Ed25519
密钥与真实 zip，调用正式签名工具，验证原始字节签名、单字节篡改和错误公钥拒绝，并串联 core/Tauri 验签测试。
临时私钥默认在结束时删除；`--keep-data` 只用于本地诊断，保留目录不得发布。

官方仓库不收 `user-declared` 插件；`official-free + acquire` 仍需单源授权审核，当前官方仓库安装流继续拒绝。

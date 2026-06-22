# 源插件 SDK(契约 v0.1)

> 运行时尚未实现(排期 v0.7)。本目录先冻结**契约**:manifest 格式、宿主 API、
> 插件接口。契约先行的目的是让运行时实现与社区插件可以并行起步。
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
| `host-api.d.ts` | 插件能用的全部宿主能力:`host.http` / `host.html` / `host.kv` / `host.log` |
| `source-plugin.d.ts` | 插件必须实现的接口:`search` / `getBook` / `getChapter` |

`manifest.capabilities` 只声明可选能力:`browse`、`resolveUrl`、`fetchMetadata`、`acquire`。
宿主会根据 manifest、授权性质和 ToS 门控决定是否显示/放行这些能力；插件返回值不能自行决定可缓存正文。

## 安全模型(写插件前必读)

- 插件跑在沙箱 JS 引擎里(桌面/Android/鸿蒙:QuickJS;iOS:JavaScriptCore)。
- 没有 DOM、Node、fetch、文件系统;能力只来自 `host`。
- `host.http` 只放行 `manifest.domains` 列出的域名。
- ES2020 纯 JS 子集;不要依赖任何浏览器/Node 全局(`URL` 等由宿主按 API 版本提供)。

## 合规边界

官方插件仓库只收 `legal.kind` 为 `public-domain` / `open-license` / `official-free`
的源,且 `official-free` 必须遵守源站 ToS(含抓取频率)。内核不内置任何源。
`user-declared` 插件只能用户自装,安装时必须做明示确认,UI 上也必须与官方插件区分。

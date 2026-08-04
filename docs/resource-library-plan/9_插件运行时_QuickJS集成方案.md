# 插件运行时：QuickJS 集成方案（v0.7）

> 范围声明：本文件只设计"如何把已有的 DTO/策略门（`plugin_manifest` / `plugin_package` / `plugin_store` / `plugin_repository` / `plugin_host`）接上一个真正执行 JS 的引擎"。不重新讨论"要不要插件运行时"——那是 `7_终局架构_多端与插件运行时.md` 已经定的结论（双引擎：桌面/Android/鸿蒙用 QuickJS，iOS 用 JavaScriptCore，过审约束 2.5.2）。本文聚焦 v0.7 的桌面 QuickJS 落地，并为 v0.9 的引擎切换留好边界。
>
> 本文最初是纯方案设计。2026-07-20 实现复核后，桌面端已按本方案接入 QuickJS；实际代码以
> `crates/reading-core/src/plugin_runtime.rs` 和 `src-tauri/src/plugin_executor.rs` 为准。当前采用同步 Rust 回调 + JS Promise
> 包装，整个 Runtime 在 Tauri `spawn_blocking` 中执行，不让 QuickJS Context 跨 Rust async await 点；SDK 可见语义仍是 Promise。

## 〇、现状盘点（决策的起点）

读完现有代码后确认的事实，所有决策都建立在这些事实上：

- `plugin_host.rs` 当前**不执行任何 JS**，只是策略门：`ensure_method_allowed` / `plan_http_get` / `ensure_kv_access` / `authorize_acquire_proposal`。它的输入输出已经是稳定的 serde DTO（`PluginSearchRequest`、`PluginBookDetail`、`PluginChapterContent`、`HostHttpGetRequest`/`HostHttpGetPlan` 等），命名用 `camelCase` 走 JSON。
- `plugin_manifest.rs` 已经把权限模型钉死：`permissions: [http, kv]` 控制能不能调用对应 host 函数；`capabilities: [browse, resolveUrl, fetchMetadata, acquire]` 控制能不能调用可选方法；`domains[]` 是 HTTP 出站的唯一白名单，`is_url_allowed_by_manifest` 是精确域名匹配（不含子域）。
- `plugin_package.rs` / `plugin_store.rs` 已经把"插件 = 一个目录，含 `manifest.json` + 单文件 `entry`（`.js`，无路径分隔符）+ `install.json`"的磁盘布局钉死。**入口脚本本身已经是纯文本字符串存在磁盘上**，QuickJS 集成只需要读它、喂给引擎，不需要改装包/安装格式。
- `plugin-sdk/host-api.d.ts` 已经把 host API 的 TS 类型面定稿：`host.http.get`、`host.html.parse/select/selectFirst`、`host.kv.get/set/delete`、`host.log.*`。`source-plugin.d.ts` 定了插件必须导出的方法集（`search/getBook/getChapter` 必选，`browse/resolveUrl/fetchMetadata/acquire` 按 capability 可选）。
- reading-core 目前没有任何 JS 引擎依赖，也没有 HTML 解析 crate（`scraper`/`html5ever` 均未引入）；`reqwest` 只存在于 `src-tauri`（壳层），core 本身"无网络"是当前架构纪律（见 `4.1` 节）。这意味着 QuickJS 集成必须自己决定：HTTP 到底是该让 core 直接发，还是继续经壳层转发。本文第五节会处理这个边界。

这份盘点决定了下文的每一个"为什么"：DTO 已经存在 → QuickJS 只是新增一层序列化适配，不是重新设计协议；权限模型已经存在 → QuickJS 沙箱只需要"绑哪些函数"，不需要重新发明权限检查；HTTP 不在 core → 需要决定调用栈穿不穿 Tauri command。

## 一、QuickJS 选型

### 候选对比

| 候选 | 本质 | Windows 编译 | 维护状态 | API 形态 |
|---|---|---|---|---|
| `rquickjs` | 对 quickjs-ng（QuickJS 的社区延续 fork）的高层 Rust 绑定，提供 `Runtime`/`Context`/`Function`/`Promise` 等安全封装，支持 async | 用 `cc` crate 编译 C 源码；quickjs-ng 官方已验证 MSVC + MinGW 均可编译；不依赖 POSIX 专有头 | 活跃，2024-2025 持续发版，是目前 Rust 生态事实标准 | 高层、符合 Rust 习惯（`Object`/`Array`/`Function` 都有安全包装），内建 `async`/`Promise` 桥接 |
| `quickjs-rs`（原始绑定，如 `quick-js`/`rust-quickjs`） | 对原始 Bellard QuickJS 的薄绑定 | 部分库历史上依赖 `cc` + Unix-only 构建脚本片段（如 `alloca.h`、`pthread`），在 MSVC 下需要打补丁；社区报告过 MinGW-only 可用、MSVC 编译失败的 issue | 多数处于半放弃状态，最后发版停留在 2021-2022，原始 QuickJS（非 -ng fork）本身也已停更两年以上 | 低层，C 指针味重，无原生 Promise 支持，需要手写事件循环胶水 |
| 嵌入式 QuickJS 原生绑定（自己写 `build.rs` 链 quickjs-ng C 源码，不经任何现成 crate） | 完全自控的 FFI | 可行，但等于把 `rquickjs` 内部已经做过的事重新做一遍 | 维护负担全部自己背 | 需要自己设计安全层（生命周期、GC 根、字符串编码） |

### 决策：`rquickjs`，绑定 quickjs-ng 而非原始 QuickJS

理由：

1. **Windows 兼容性是一票否决项**——本项目桌面优先目标是 Win/Mac/Linux 三端同时打包，且 CI/开发机当前环境就是 Windows。`rquickjs` 选择 quickjs-ng 作为底层（quickjs-ng 是 Bellard 原始 QuickJS 因长期不维护而产生的社区延续分支，目前是事实上的"在用"版本），quickjs-ng 自身的构建脚本已经处理了 MSVC ABI 差异（如 `__builtin_*` 在 MSVC 下的替代、`alloca` 的 Windows 等价物），不需要我们手动打 patch。原始 `quickjs-rs` 类绑定多数还停留在绑定 Bellard 原版 QuickJS，那个版本的 Windows 支持是"社区维护一半"的状态，存在编译失败的已知 issue。
2. **省掉一整层胶水**：`rquickjs` 原生提供 `Promise`/`async fn` 互转，host 函数可以直接 `async fn(...) -> Result<T>` 注册，运行时自动包成 JS `Promise`。这正好对应 host API 设计——`host.http.get`/`host.kv.get` 在 TS 类型里都是 `Promise<T>`。如果用低层绑定，这层异步桥接要自己手写一遍 microtask 队列，等于重新发明 `rquickjs` 已经做好的轮子。
3. **体积**：quickjs-ng 编译后在 700KB 量级（`7_终局架构` 文档里写的数字与此一致），符合"体积极小"硬约束；不会把 `rquickjs` 误认为是"又一个大引擎"。
4. **不选嵌入式自写 FFI**：唯一的优势是"完全可控"，但代价是要重新实现 `rquickjs` 已经趟过的坑（GC 根管理、`Value` 生命周期、字符串编码转换、错误传播），团队规模（个人+AI 协作）下这是负成本投入。仅在 `rquickjs` 出现无法绕过的阻塞（例如未来发现它在某个目标三元组上彻底编译不出）时才考虑这个后备方案。

不选 `quickjs-rs`（低层绑定）的根本原因：它绑的是停更的原始 QuickJS，而不是仍在修 bug、修安全漏洞的 quickjs-ng；用一个停更引擎去跑"用户会安装第三方代码"的沙箱，本身是个安全隐患的开端。

### 与 iOS JavaScriptCore 的边界

`rquickjs` 只编译进桌面/Android/鸿蒙的 reading-core 构建（条件编译，例如 `#[cfg(not(target_os = "ios"))]` 或者更干净地用 Cargo feature `quickjs-runtime`）。iOS 目标下 reading-core 完全不链接 QuickJS，引擎调度改为壳层调 JavaScriptCore（`JSContext`/`JSValue`，Swift 侧）。这是为什么第二节要把"host API 调用约定"和"JS 引擎"彻底解耦——引擎换了，host 函数签名和 DTO 协议不能换一个字。

## 二、沙箱边界

### 全局对象清零策略

`rquickjs` 的 `Context` 默认带 QuickJS 内建的全局对象（`Object`/`Array`/`JSON`/`Math`/`Promise` 等标准库,这些必须保留，插件 JS 离不开它们)。需要显式**不**注入的是 QuickJS 可选模块：

- 不启用 `std`/`os` 模块（quickjs-ng 把文件 I/O、进程、`exec` 都封装在这两个可选内建模块里，默认 `Context` 不包含，必须显式 `Module::evaluate` 才能用——v0.7 集成代码永远不调用这条路径）。
- 不注入 `fetch`/`XMLHttpRequest`/`WebSocket` 等网络全局——QuickJS 本身没有这些，不会"意外带出来"，但要确保我们自己写的 polyfill 代码里不会手滑加上。
- 不暴露 `eval`（QuickJS 标准里 `eval` 默认存在；需要在 `Context` 初始化后立即 `delete globalThis.eval`，或用 quickjs-ng 的编译期开关 `-DCONFIG_DISABLE_EVAL` 等价能力，二选一，倾向运行时删除以保留对未来 QuickJS 升级的兼容性）。
- 只注入两个标准全局补丁：`URL`（host-api.d.ts 已注明）和 `TextDecoder`（处理非 UTF-8 站点如 Shift_JIS 解码后的字符串，这两者 QuickJS 标准库不自带，必须由宿主用 Rust 实现后挂上去，行为在 QuickJS 和 JavaScriptCore 两边必须一致——这是双引擎契约的一部分）。

### `host` 对象 = 唯一出口

插件 JS 能触达的宿主能力，严格等于 `host-api.d.ts` 列出的四个命名空间：`host.http.get`、`host.html.parse`、`host.kv.get/set/delete`、`host.log.*`。集成层的职责是把这四组函数注册为 `rquickjs::Function`，**每一个函数体内部都重新调用一次 `plugin_host.rs` 现成的策略门**，不是"在注册时检查一次权限就完事"：

- `host.http.get` → 内部调用 `plugin_host::plan_http_get(manifest, request)`，拒绝时把 Rust `Err(String)` 转成 JS `Promise` reject，插件侧用 `try/catch` 或 `.catch()` 拿到错误信息。
- `host.kv.get/set/delete` → 内部调用 `plugin_host::ensure_kv_access(manifest, key, value)`。
- `host.html.parse` 不经过权限门（解析能力不依赖网络/存储权限，纯计算），但要对输入大小做限制（防止插件喂入超大字符串触发 host 侧 HTML 解析器的 CPU/内存放大攻击——这是新引入的攻击面，因为目前 reading-core 没有 HTML 解析 crate）。
- `host.log.*` 无权限门，但必须限制单次调用的参数总长度（例如 4KB），防止插件用日志当作绕过 kv 容量限制的存储通道，或者用日志刷爆磁盘。

**为什么每次调用都重新查权限，而不是初始化时一次性决定"这个插件能不能用 http"**：因为 `domains[]` 白名单是按 URL 校验的，不是按"有没有 http 权限"校验的——同一个插件对 `manifest.domains` 内的 URL 能发请求，对外面的 URL 不能。权限检查的粒度天然就是"每次调用"，这与现有 `plugin_host.rs` 的设计完全一致，QuickJS 集成不应该、也不需要引入一个新的粗粒度缓存层。

### 明确不暴露的能力（沙箱边界之外的一切）

不做 allowlist 之外的任何"看起来安全所以加一个"的全局：不暴露 `host.fs`（没有文件系统能力，插件偷不走书库文件、写不进任意路径）；不暴露任意网络（没有裸 `fetch`，只有受 `domains[]` 约束的 `host.http.get`，且只有 GET，没有 POST/PUT——v0.7 host API 草案就是只读抓取，写操作不在范围内）；不暴露 `host.process`/`host.exec`；不暴露动态代码加载（插件不能 `import()` 第二个脚本，一个插件 = 一个 entry 文件，`plugin_package.rs` 已经在安装时把 entry 限制为单文件、无路径分隔符，QuickJS 侧呼应这一点：永远只 `eval` 这一份已读入内存的源码字符串，不允许它在运行期再去读其它文件）。

## 三、生命周期：加载 / 运行 / 卸载，崩溃隔离

### 一次调用 = 一个 `Runtime` + 一个 `Context`

`rquickjs::Runtime` 持有引擎级状态（内存分配器、GC），`Context` 是其中跑代码的环境（全局对象、模块系统）。设计为：**每次方法调用（`search`/`getBook`/`getChapter`/`browse`/...）新建一个 `Runtime`+`Context`，跑完即弃**，不维护跨调用的常驻插件进程/线程。

理由与权衡：

- **隔离性最强、实现最简单**：一个插件这次调用里写了死循环或者内存暴涨，损失的只是这一次 `Runtime`，丢弃后内存立即释放（`rquickjs::Runtime` 的 `Drop` 会回收 QuickJS 堆），不会污染下一次调用，也不会拖累其它插件——天然满足"一个插件崩不影响其他"的要求，且不需要写任何额外的状态恢复逻辑。
- **代价是丢失跨调用的内存态**（比如插件想在 JS 闭包里缓存上次搜索的中间结果）。这正好是 `host.kv` 存在的原因：跨调用状态必须经 `host.kv`（落盘、容量受限、按插件 id 隔离），不允许活在 JS 堆里。这与 host-api.d.ts 的注释（"用于缓存 token、目录页等"）完全吻合，不是新增约束，是确认现有设计已经覆盖了这个需求。
- **不做"常驻插件进程/Worker 池"**：常驻意味着要解决"插件 A 的全局状态会不会污染插件 B"（答案是 per-Runtime 不会，但常驻还要解决"什么时候回收空闲 Runtime""一个 Runtime 复用多次后内存只增不减怎么办"），这些问题在一次性 Runtime 模型下根本不存在。v0.7 阶段插件调用频率低（用户主动搜索/翻章节，不是持续轮询），一次性创建的开销（quickjs-ng 创建 `Runtime` 是毫秒级）完全可以接受，不值得为省这点开销引入状态管理复杂度。

### 内存限制

`rquickjs::Runtime::set_memory_limit` 在创建时设置硬上限（建议初始值 64MB，足够 HTML 解析中间产物 + JSON 往返，又能防止恶意插件用字符串拼接攻击吃光宿主内存）。超限时 QuickJS 会让分配失败、当前执行抛 JS 异常，集成层捕获后转成"插件内存超限"错误返回给调用方，不让进程崩溃。

### 崩溃隔离的真实含义

quickjs-ng 是纯 C 解释器，不会像"调用一个有未定义行为的原生扩展"那样让宿主进程 segfault——这是选择脚本引擎（而不是让插件提供原生动态库）的根本原因之一。崩溃隔离在这个模型下实际上靠两层东西，不是靠"进程隔离"：

1. 内存上限（防止 OOM 拖垂宿主）；
2. 超时机制（见第四节，防止死循环/死等）。

不引入"每个插件一个子进程"的隔离方式——子进程方案能拿到更强的隔离（OS 级），但代价是要重新设计 IPC 来传递 `host.http`/`host.kv` 调用（子进程没有直接访问 reading-core 内部状态的能力，所有 host 调用都要走一轮序列化往返），复杂度和延迟都不成比例地增加，而 QuickJS 本身是内存安全的解释器，原生扩展级别的崩溃风险并不存在。v0.7 不做这个，留作风险登记项（见第七节）。

### 卸载

"卸载"在一次性 Runtime 模型下是空操作——没有常驻状态需要清理。真正对应"卸载插件"的操作仍然是 `plugin_store::uninstall_plugin`（删除磁盘目录），QuickJS 集成层不需要新增任何卸载钩子。唯一需要注意的是：**正在执行中的调用如果此时插件被禁用/卸载，不强行中断**，让本次调用跑完或超时，下一次调用前重新检查 `ensure_method_allowed`（已有逻辑，校验 `enabled` 字段）即可阻止后续调用——避免在调用中途中断 Runtime 引入新的状态不一致问题。

## 四、调用模型：异步、单线程、超时

### 单线程执行模型

QuickJS 本身是单线程解释器（没有内建多线程支持，这也是它体积小的原因之一）。一次插件方法调用（如 `getChapter`）内部可能有多次 `await host.http.get(...)`，这些 `await` 在 QuickJS 里通过它自己的 microtask 队列驱动，`rquickjs` 把这套机制和 Rust 侧的 `async`/`Future` 桥接：host 函数注册为返回 `impl Future` 的 Rust 函数，`rquickjs` 自动包装成 JS `Promise`，插件 `await` 它时，QuickJS 让出执行，Rust 侧真正去发 HTTP 请求，请求回来后 resolve 这个 `Promise`，QuickJS 恢复执行剩下的 JS 代码。

这意味着**调用 reading-core 暴露的"执行插件方法"接口本身必须是 async**（例如 `async fn run_search(plugin, request) -> Result<PluginSearchPage, String>`），调用方（Tauri command）`await` 它即可，内部的 QuickJS 事件循环驱动对调用方透明。

### 超时机制：30 秒硬上限

设计为两层超时，而不是单一层，因为"JS 在跑死循环"和"JS 在等一个迟迟不来的 HTTP 响应"是两种不同的失控模式，需要两种不同的处置：

1. **CPU 时间片中断**：`rquickjs`（经 quickjs-ng 的 `JS_SetInterruptHandler`）支持设置一个中断回调，QuickJS 解释器会定期（按字节码执行步数）调用它；回调里检查"从本次调用开始有没有超过 30 秒"，超了就返回让解释器立即抛异常终止执行。这一层专门治"插件写了 `while(true){}`"——纯 CPU 死循环不会主动让出，必须靠解释器自己定期检查中断标志才能打断。
2. **host 函数级超时**：`host.http.get` 内部的真实网络 I/O 受 `HostHttpGetPlan.timeout_ms`（已有字段，默认 15s、上限 60s）控制，但这是"单次 HTTP 请求"的超时，不是"整次插件调用"的超时——插件可能连续发 5 次 HTTP 请求，每次 14 秒，单次都不超时，但总和远超 30 秒。所以还需要在"执行插件方法"这个外层包一个 `tokio::time::timeout(Duration::from_secs(30), run_plugin_method(...))`，这层管的是整次调用的墙钛时间预算，与 CPU 中断互补（一个管纯计算超时，一个管"计算+IO 总和"超时）。

两层都触发同一个结果：返回结构化错误（例如 `"plugin call exceeded 30s timeout"`），不让调用方永久挂起。`30s` 的数字来自任务要求里的"单次调用上限"，与现有 `MAX_HTTP_TIMEOUT_MS = 60_000`（单次 HTTP 请求上限）不冲突——一次插件方法调用允许的总时间（30s）比单次 HTTP 上限（60s）短，意味着如果插件配置了一个接近 60s 的超时去发请求，外层 30s 会先触发，这是有意为之：HTTP 层的 60s 上限是"防止插件把单次请求超时设得离谱"的输入校验，调用层的 30s 是"防止插件拖死整个搜索/翻页体验"的产品级体验保证，两者服务不同目的，不需要对齐成同一个数字。

### 不做插件间并发限流（v0.7 范围声明）

如果用户同时触发多个插件调用（例如搜索时多个源并发查询），每次调用各自起一个 `Runtime`，天然并发安全（不共享可变状态）。是否要限制"同时跑几个插件 Runtime"是资源调度问题，不是沙箱安全问题，v0.7 不做限流，观察实际使用模式后再决定要不要加并发上限。

## 五、与现有 DTO 对接：序列化层设计

### 序列化协议：JSON 字符串作为唯一边界，`serde_json` 两端复用

`plugin_host.rs` 的所有 DTO（`PluginSearchRequest`/`PluginSearchPage`/`PluginBookDetail`/`PluginChapterContent`/`HostHttpGetRequest`/`HostHttpGetPlan`/`AcquireProposal`/`AcquireHostDecision` 等）已经是 `#[derive(Serialize, Deserialize)]` + `camelCase`。集成层的转换规则统一为：

**Rust → JS**：`serde_json::to_string(&dto)` 得到 JSON 字符串 → 用 `rquickjs::Context` 的 `JSON.parse` 等价能力（`rquickjs` 提供 `Value::from_json`-类辅助，或直接把 JSON 字符串作为参数传入、在 JS 侧用全局 `JSON.parse` 还原）转成 JS 对象，再作为插件方法的入参（例如 `search(query, page)` 的两个标量参数直接传，不需要包成对象；但 `getBook`/`getChapter` 的返回值是结构体，走 JSON 往返）。

**JS → Rust**：插件方法 resolve 出的 JS 对象，用 `JSON.stringify` 转回字符串，再 `serde_json::from_str::<PluginBookDetail>(...)` 反序列化回 Rust 类型，复用已有 DTO 定义和它们已经写好的字段校验（`#[serde(default)]` 对可选字段的处理已经在 DTO 里定义好了，不需要在 QuickJS 层重新写）。

理由：**始终走 JSON 文本往返，不走"Rust 侧手动遍历 JS Object 取字段"的细粒度 FFI**。原因是 `rquickjs` 的细粒度 `Object::get::<_, String>("title")` 这类调用对每个字段都要过一次 FFI 边界，DTO 字段一多就是几十次调用；而 JSON 字符串往返是两次 FFI 调用（一次传入、一次取出）+ 纯 Rust 侧的 `serde_json` 解析，后者是非常成熟、零额外信任风险的路径。代价是多一次 JSON 序列化/反序列化的 CPU 开销，但插件返回的数据量级（搜索结果页、单章正文文本）远低于会让这点开销变成瓶颈的规模。

这个决定的另一个好处：**JS → Rust 反序列化失败（插件返回的对象缺字段/类型不对）会被 `serde_json` 的错误自然捕获**，转成"插件返回数据格式不符合契约"的错误返回给调用方，不需要在 QuickJS 集成层手写一套 JS 值的结构校验——`source-plugin.d.ts` 定义的契约由 Rust 侧的 DTO 结构本身充当运行时校验器。

### 调用编排：一次方法调用的完整链路

以 `getChapter(chapterUrl)` 为例，串起所有现有模块：

1. 调用方（Tauri command）传入 `plugin_id` + `chapterUrl`。
2. 从 `plugin_store::list_installed_plugins` 加载到 `InstalledPlugin`（含 `manifest`），调 `plugin_host::ensure_method_allowed(plugin, SourcePluginMethod::GetChapter)` 校验启用状态/capability（已有逻辑，不用动）。
3. 从磁盘读取该插件的 entry `.js` 源码文本（`plugin_store` 已确定的目录布局：`<plugin_root>/<id>/<entry>`）。
4. 新建 `rquickjs::Runtime`+`Context`，设置内存上限、删除 `eval`、注册中断处理器，挂上 `host` 对象（`host.http.get` 闭包捕获了 `manifest` 的克隆，每次调用内部重新跑 `plan_http_get`）。
5. `eval` 插件源码，取出 `export default` 的对象上的 `getChapter` 函数，传入 `chapterUrl`，`await` 返回的 `Promise`。
6. 拿到 JS 端 resolve 出的对象，`JSON.stringify` → `serde_json::from_str::<PluginChapterContent>`。
7. `PluginChapterContent.html` 交给 reading-core 已有的 HTML 清洗器（`epub` 模块的清洗逻辑，复用，不重写）做二次净化（`source-plugin.d.ts` 注释里已经写明"宿主入库前还会过 reading-core 的 HTML 清洗"——这是已有承诺，QuickJS 集成只是让这个承诺第一次有了真正的执行路径）。

### HTTP 出站的边界问题：必须解决，不能拖到实现期

现有架构纪律是"core 无网络，网络在壳层"（`reqwest` 只在 `src-tauri`）。但 `host.http.get` 的策略门 `plan_http_get` 在 `plugin_host.rs`（core 里），它只做"计划"（校验+清洗后的请求参数），不做"执行"。QuickJS 跑在 core 内部，`host.http.get` 这个 JS 函数被调用时，谁真正去发这个 HTTP 请求？

两个选项：

- **A. core 内部直接发**：给 reading-core 加 `reqwest`（或更轻的 HTTP 客户端）依赖，违反"core 无网络"纪律，但避免一次跨壳往返。
- **B. core 把"计划"通过回调/channel 抛回壳层执行，结果再喂回 QuickJS 的 Promise**：保持纪律，但要设计一个"QuickJS 调用挂起 → 等壳层异步回调 → resume Promise"的桥接通道。

**决策：选 B，新增一个 HTTP 执行的回调接口（trait），由壳层注入实现**。reading-core 定义一个 trait（例如 `trait PluginHttpExecutor { async fn execute(&self, plan: HostHttpGetPlan) -> Result<HostHttpResponse, String>; }`），`host.http.get` 内部调用这个 trait 对象，而不是直接 `reqwest::get`。Tauri 壳层用已有的 `reqwest`（已经在 `src-tauri/Cargo.toml`）实现这个 trait 并注入。这保留了"core 不直接依赖具体 HTTP 客户端"的纪律（依赖倒置：core 定义接口，壳层提供实现），同时不需要插件运行时本身跨进程——`Runtime`/`Context` 仍然活在同一个进程里，只是 HTTP 执行的"最后一跳"经过一层 trait 调用，不是物理上的 IPC。这与 `7_终局架构` 文档里"逻辑下沉 Rust，但 reqwest 在壳层"的既有纪律完全一致，QuickJS 集成不需要、也不应该打破它。

这个设计同时也为 v0.8/v0.9 的 Android/iOS 移植铺路：Android 壳和 iOS 壳各自实现一份这个 trait（用各自平台原生的 HTTP 客户端或继续用 `reqwest`），reading-core 侧的 `plugin_host.rs` 一行不用改。

### `host.html.parse` 的实现位置：新增 core 内部依赖，不下放壳层

和 HTTP 不同，HTML 解析是纯计算（没有网络/文件 I/O 副作用），不违反"core 无网络"纪律，应该直接在 reading-core 内实现，新增一个 HTML 解析 crate 依赖（如 `scraper`，构建在 `html5ever` 之上，支持 CSS 选择器查询，与 `host-api.d.ts` 里 `select(selector)`/`selectFirst(selector)` 的语义直接对应）。`HtmlDoc`/`HtmlElement` 是 QuickJS 侧的包装对象，内部持有 `scraper::Html`/`ElementRef` 的 Rust 引用，`select`/`text`/`attr`/`innerHtml` 都是直接转发到 `scraper` 的查询 API，不需要二次设计解析逻辑。

## 六、插件 SDK 的配合更新

`plugin-sdk/host-api.d.ts` 和 `source-plugin.d.ts` 当前是**类型声明**（给插件作者在编辑器里写代码用，没有运行时实现）。QuickJS 集成落地后，SDK 需要配合做的事：

1. **类型声明本身基本不用改**——它已经按"引擎无关的纯 JS 子集"设计（无 DOM、无 Node API），这正是为 QuickJS/JavaScriptCore 双引擎兼容预留的设计，QuickJS 落地不会推翻这份契约，只是第一次有了真正兑现它的引擎。唯一可能要补的是把"哪些 ES2020 特性双引擎都保证支持"写成一份显式 polyfill/兼容性清单（例如：QuickJS 对 `Array.prototype.flatMap` 等较新方法的支持版本要核实是否与目标 JavaScriptCore 版本一致），避免插件作者写出"在 QuickJS 能跑、在 iOS 上 JavaScriptCore 因为版本差异跑不动"的代码。
2. **示例插件骨架需要做一次真实验证**——`examples/aozora-bunko/plugin.js` 当前的注释明确写着"接入真实运行时(v0.7)前需用实际页面验证"，QuickJS 集成落地是这句注释指向的那个时间点：需要用真实 HTTP 响应跑一遍 `search`/`getBook`/`getChapter`，确认 CSS 选择器和 URL 模板对得上线上页面结构，把骨架升级成真正能用的示例（这是 QuickJS 集成验收的一部分，不是事后可选项——光有引擎没有一个跑得通的真实插件，等于没有验证沙箱设计是否可用）。
3. **SDK 需要新增"本地试跑"开发者体验**（建议但非阻塞项）：插件作者目前没有办法在不安装进真实应用的情况下验证自己的脚本。可以在 `plugin-sdk/` 下补一个轻量 Node 脚本，用 mock 的 `host` 对象（同样的接口形状，但 `http.get` 打真实 fetch、`kv` 用内存 Map）跑插件的三个必选方法，帮插件作者在提交到官方仓库前自己跑通——这不是 QuickJS 集成的必要部分，但能大幅降低插件作者的调试成本，建议排进 v0.7 范围。
4. **`manifest.schema.json` 不需要因为 QuickJS 集成而改字段**——执行引擎是运行时细节，不影响 manifest 声明的权限/能力模型,这层已经设计对了。

## 七、明确不做什么（范围边界）

- **不引入 `deno_core`**：`deno_core` 是为"跑接近 Node.js 能力面的 JS"设计的（内建模块系统、`Deno.*` 全局、V8 而非 QuickJS），体积（V8 几十 MB 起）直接违反"体积极小"硬约束，且能力面远超"几个 host 函数"的沙箱需求——选它等于把攻击面从"四个白名单函数"扩大到"几乎整个 Deno 运行时"，与沙箱设计目标背道而驰。
- **不引入 `boa`**（纯 Rust 实现的 JS 引擎）：优势是没有 C 依赖、跨编译简单，但成熟度和性能不如 quickjs-ng,且不存在"iOS 上有官方对应物"这件事——选它意味着桌面/移动两套引擎完全不同源，双引擎契约的"行为一致性"风险更高。quickjs-ng 和 JavaScriptCore 至少都是经过多年 Web 生产环境验证的 ES 实现，行为一致性的基准线更可信。
- **不引入任何浏览器引擎/WebView 跑插件 JS**：插件 JS 不需要 DOM、不需要渲染，用 WebView 跑纯逻辑脚本是资源浪费,且会引入"插件能不能拿到 WebView 里的东西"这类全新的沙箱漏洞面。
- **不执行第三方抓取正文之外的任何代码**：host API 没有 `eval`、没有动态 `import`、没有第二个脚本文件入口；插件能做的事被收得只剩"用 `host.http.get` 拿文本、用 `host.html.parse` 抽取字段、把结果按 DTO 形状返回"，再没有别的执行面。这是对 `7_终局架构` 文档里"内核零内置源、插件偷不走书库、打不了内网"承诺的具体落地,QuickJS 集成的每一个决策都应该回头检查是否仍然满足这条线。

## 八、风险登记（补充 `7_终局架构` 已有风险表）

| 风险 | 缓解 |
|---|---|
| quickjs-ng 在某个 Rust 目标三元组（尤其鸿蒙 `aarch64-unknown-linux-ohos`）上编译失败 | v0.7 只做桌面验证；Android/鸿蒙编译验证作为 v0.8/v1.0 前置检查项，发现问题时优先反馈给 `rquickjs` 上游而不是 fork 维护 |
| HTML 解析 crate（`scraper`/`html5ever`）处理恶意构造的畸形 HTML 时性能退化（ReDoS 类问题，CSS 选择器引擎也可能有最坏情况复杂度） | `host.html.parse` 入口处加输入长度上限；超时机制（第四节的 30s 总预算）兜底捕获病态输入 |
| JSON 往返序列化在插件返回超大正文（长篇章节）时的内存/CPU 开销 | 已有 `MAX_KV_VALUE_LEN`（64KB）量级提示了"单次有效载荷"应该多大；评估是否需要给 `PluginChapterContent.html` 也加一个上限，超限直接拒绝而不是吃满内存做一次序列化 |
| 一次性 `Runtime` 模型在高频调用场景（如批量导入多本书）下的创建开销累积 | v0.7 不做池化；先用真实场景测量,只有测出瓶颈才引入 Runtime 复用,不预先优化 |
| HTTP 执行回调（trait 设计）在 Android/iOS 移植时需要各自实现一份 | 这正是该设计的目的（依赖倒置换来的移植收益），但需要在 v0.8 启动时确认 trait 接口形状足够平台中立，不带任何 Tauri/`reqwest` 特有类型泄漏进 core 的公共签名 |

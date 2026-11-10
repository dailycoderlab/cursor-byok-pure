# Cursor BYOK Pure 项目完整性、外联与安全审查

> 首次审查日期：2026-11-05（Asia/Shanghai）。**第 1–9 节保留改动前下载快照的结论，不代表当前实现。** 原始 605 文件指纹与当时“无 Git/无 Rust 工具链、未安装前端依赖”的记录仍属于首次任务。当前行为见[纯净模式说明](<./PURE-MODE-CN.md>)，当前结构见[源码阅读指南](<./CODE-READING-CN.md>)。

## 当前改动摘要（历史报告之前）

**已移除广告、作者公共 TAB/Commit 默认外联与上游更新源；并非所有安全问题已修复。** 当前已有 Git 基线 `6a13780 Initial commit`，实现处于 dirty working tree；本次助手没有 commit/push。以下旧快照的数字、证据和当时执行范围不作为当前构建/测试状态。

| 历史项目 | 当前状态 |
| --- | --- |
| S1 公共 TAB/默认 Commit | 原 Public 模式/默认作者目标已删除；Local 默认且必须明确选模型，空选择停用无外部回退。Commit 独立：选模型本地生成，未选官方直连，不跟随 TAB |
| S6 广告图片 SSRF | 广告模块/下载/加载路径已删除，相关入口消失；不等于其他 HTTP 入口都完成安全修复 |
| 广告与推广 | 拉取、安装 ID 创建、关闭反馈、前端 UI/cache/localStorage、相关翻译与 marquee 依赖移除；历史真实用户数据未删除、不读复用 |
| 上游更新 | **保留 updater 功能**；Tauri 源为空、Windows portable 源为 None，自动短路，手动/缓存安装提示“暂无更新源”；签名公钥/机制保留，未来需自己的源与正确签名 |
| S2、S3–S5、S7–S13、Agent 正确性缺口 | **未由此次纯净化修复**；管理无鉴权、OAuth/日志/插件删除/Semble 等原结论仍须单独处理 |

前端 TypeScript/Vite/i18n 检查、最终隔离单线程 Rust 全 workspace **332 项**、macOS debug 无 bundle 构建均通过，执行条件和首次并发传输断言失败见[当前验证表](<./PURE-MODE-CN.md#6-验证结果与未验证项>)。**未运行真实桌面或 Cursor E2E，也未做 Windows/Linux 原生构建。** 后台本地 TAB 不依赖官方账号，但官方 Direct 仍需登录/额度，模型 API/其他 RPC 仍可外联；**Pure 不等于 offline 或 secure**。

### 如何读取历史证据

历史源码以 Git 基线读取，不把当前已修改文件的行号冒充旧证据。下文的 `git show` 片段是只读定位方式；历史行范围可在该输出中查看。已删除模块不保留 dead link。未改动源码的普通链接仅方便导航，当前实现仍以当前文档为准。

```bash
# 仓库根目录；只读显示基线内容，不切换或覆盖工作区。
git show 6a13780:server/src/cursor/services/tab.rs
git show 6a13780:server/src/control/ads.rs
git show 6a13780:server/src/store/settings.rs
git show 6a13780:apps/desktop/src/shell/AppLayout.tsx
git show 6a13780:apps/desktop/src-tauri/tauri.conf.json
```

## 1. 历史结论（改动前）

### 1.1 项目完整吗？

**作为 Cursor BYOK Pure 本地模型网关，主体实现较齐全，不是只有转发接口的空壳；但不是完整 Cursor 官方后端，也有明确的功能、正确性、安全和打包欠缺。**

- 有桌面应用、本地服务、三种模型协议、Agent 工具循环、会话/checkpoint、取消/续聊/恢复/压缩、模型设置、调用记录及测试。
- 未接管 RPC、官方模型、部分账号/模型目录请求仍使用 Cursor 官方服务；TAB 并未在本地实现，而是透传。
- “任意自己的 API Key”应理解为“配置兼容服务的地址、协议、模型 ID 与凭据”，**不是任意厂商原生协议、任意模型能力都能直接使用**。
- 静态检查没有发现 Rust 常规模块或字面量 include 缺文件；但没有编译 Rust 或构建桌面，不能保证当前代码全部可构建。
- 辅助抓包工具依赖的 Go protobuf 生成包没有包含在当前目录，需要额外生成；Docker 构建阶段也没有复制 workspace 必需的 Semble 成员目录。不能称所有部署路径开箱可构建。

### 1.2 有暗门或广告吗？

**有明确广告、作者公共服务外联、自动更新检查和额外搜索/OAuth 请求。不符合“纯本地、只访问我自己模型 API”的要求。**

尤其需要关注：

1. **默认 TAB 服务是作者的 `https://tab.leokun.cn`**，16 个透传路由包括文件同步/上传和代码指标。
2. **默认提交信息生成也跟随 TAB 设置**：未选择本地 Commit 模型时，diff 请求默认同样发往作者公共服务，而不一定是官方。
3. 这些透传请求的原始 Authorization/Cookie 等没有按第三方目标过滤；使用官方账号时，若 Cursor 请求带 token，这些凭据会一并转发给公共服务。
4. 广告会发送持久安装 ID、系统、版本、语言、禁用广告列表；关闭广告还会发送关闭原因。随机 ID 不是“每次请求匿名无关联”。
5. 本地管理接口无鉴权，可返回模型 Key、OAuth 资源和详细日志；插件自动刷新可能把 refresh/access token 写进日志。另有插件删除路径越界风险。

**没有找到足以确认作者有意植入“偷偷把所有模型 Key 上传给作者、挖矿、远控”的源码证据。** 这不等于不存在风险：广告是明确存在的推广与识别性外联；默认公共透传和已发现安全漏洞也不能被“未发现恶意后门”淡化。

### 1.3 当前是否建议直接使用？

**不建议把当前默认配置直接用于公司代码、生产模型 Key 或主账号 OAuth 凭据，更不能直接暴露服务端口给局域网/公网。**

建议先处理第三方透传凭据、管理鉴权、插件删除边界和日志凭据这几项，再用低权限测试 Key、非敏感仓库做实机兼容验证。本文只报告问题，未修改业务代码、依赖、已有配置或系统信任。

## 2. 审查范围与验证边界

### 2.1 本地快照

- 当前目录没有 `.git`，`git status`/`git remote` 返回“not a git repository”。不能确认它对应 GitHub 的哪次 commit，也不能证明下载后没有改动。
- 桌面 package（`git show 6a13780:apps/desktop/package.json`，历史 1–5 行） 与 Tauri 配置（`git show 6a13780:apps/desktop/src-tauri/tauri.conf.json`，历史 1–5 行） 标记版本 `1.0.1`；[server package](<../server/Cargo.toml#L1-L7>) 为 `0.1.0`。这是文件中的版本，不是已验证的 release/tag 身份。
- 使用 glob 枚举到原始 605 个文件，包括隐藏配置、协议、源码、图片、技能说明和锁文件。网络入口/敏感字段做了全树检索，关键路径逐文件审读；**没有逐行审计所有传递依赖和二进制素材**。
- 本次新增的两份文档不计入原始文件集合。原始集合指纹（对路径排序，逐项累积 `path + NUL + SHA256(file bytes).hex + LF` 后 SHA256）：

```text
8eea14bd460c12370e304c8deaa6a9385bacc70e2f4eedfb45393849cd8355d3
```

该指纹用于标识本次本地审查对象，不证明与上游一致。

### 2.2 执行的检查

| 检查 | 实际结果 | 能证明什么/不能证明什么 |
| --- | --- | --- |
| 常规 Rust `mod` 引用存在性 | 266 项，未发现缺目标 | 是路径检查；不是 Rust 编译/typecheck，宏生成与 cfg 语义不在此保证内 |
| 字面量 `include_str/include_bytes` 目标 | 11 项，全部存在 | 不是所有宏展开的编译验证 |
| npm package 与 lock 根配置 | version/dependencies/devDependencies 一致 | 未下载包、未验证包内容或 CVE |
| SQLite 迁移 | Node SQLite 3.51.2 内存新库顺序执行 0001–0009，通过；17 张业务表，foreign_key_check 无错误、integrity_check=ok | 仅新库 SQL 检查；未测试 SQLx 运行时及已有真实数据升级 |
| portable updater manifest 单测 | `node --test .github/scripts/generate-portable-update.test.mjs`：2/2 通过 | 仅 manifest 生成/输入验证；不是更新包验签或桌面升级联调 |
| Rust/前端/桌面构建和 Rust tests | 未执行 | 当前没有 cargo/rustc/deno/cmake，前端没有 node_modules；protoc 也未安装，但主服务使用 vendored protoc，不应误判为必须装系统 protoc |
| Cursor/真实 API/OAuth/广告/公共 TAB | 未执行 | 没有发送真实凭据/工作区内容，没有抓到实际网络包 |

没有启动应用、调用接管接口、安装 CA、结束 Cursor、改 Cursor 设置或安装依赖。数据库验证只在内存中，不接触已有用户数据。

## 3. 现有架构和数据所有权

```text
当前仓库
├── apps/desktop/         # React 设置/记录/广告 UI + Tauri 窗口、更新与原生权限
├── server/
│   ├── src/api/         # Cursor RPC
│   ├── src/local_app/   # 本机 CA、Cursor 代理/设置/账号接管
│   ├── src/cursor/      # Cursor 协议、上下文、工具、checkpoint 与传输生命周期
│   ├── src/run/         # 通用 Agent 模型→工具→继续循环
│   ├── src/provider/    # OpenAI Chat / Responses / Anthropic / 插件模型
│   ├── src/model/       # 通用消息、模型配置和历史投影
│   ├── src/store/       # SQLite 持久化
│   ├── src/control/     # 管理 API、广告拉取/关闭反馈
│   ├── src/plugin/      # Deno 沙箱、OAuth 与私有资源
│   ├── src/search/      # 网页抓取/23 源搜索与 Semble 适配
│   ├── plugins/         # 三个内置账号型 provider 插件
│   ├── prompt/          # 模式提示词和工具 schema
│   ├── migrations/      # 数据库完整迁移序列
│   └── tests/           # 大量模拟 Cursor/provider 的生命周期测试
├── crates/semble-core/  # 本地代码索引、embedding 与 git source
├── protocols/cursor/    # 抽取的 protobuf 定义，不代表全 service 已实现
└── support/             # 协议抽取、抓包调试和 benchmark，不随核心 Run 自动执行
```

关键目录实测：`server/src` 210 文件/47,588 行；`server/tests` 17 文件/9,020 行（含 3 个 support 文件）；`apps/desktop/src` 160 文件/22,041 文本行；`crates/semble-core/src` 30 文件/3,626 行；协议 6 文件/60,757 行。统计包含空行、注释、内嵌测试和生成文本；图片/字体二进制不算文本行，不把这些数字当人工逐行审计量。

```text
Cursor 客户端（拥有工作区、文件/终端工具、MCP、审批和 UI）
    │
    ▼ 本地信任 CA + HTTP(S) 代理
本地 Rust 服务
    ├─ 本地模型 → compile/checkpoint → RunEngine → provider → 你配置的 API
    │                                   ↑              │
    │                           工具结果/下一模型循环 ←──┘
    │                                   ↕
    │                     RunSSE 输出 ↔ Cursor 工具执行 ↔ BidiAppend
    ├─ 官方模型/未接管 RPC/部分元数据 ───────────→ Cursor 官方
    ├─ TAB + 默认直连 Commit ──────────────────→ 作者公共服务（默认）
    ├─ 广告/图片/关闭反馈 ──────────────────────→ 作者广告服务/广告图片目标
    ├─ 更新 ──────────────────────────────────→ GitHub/下载 CDN
    ├─ 批准后 WebSearch/WebFetch ──────────────→ 搜索来源/网页目标
    └─ OAuth 插件/首次 Semble 资产 ────────────→ OpenAI/xAI/Google/Hugging Face

落盘：SQLite 配置/消息/checkpoint/工具结果/调用记录；插件 JSON 账号资源；CA 私钥；缓存。
```

## 4. 实现完整性矩阵

| 功能 | 当前实现 | 限制/欠缺与证据 |
| --- | --- | --- |
| 自定义模型 | 配置地址、key、model ID、headers、额外参数，model_hash 显示到 Cursor | 仅兼容指定协议，不自动理解任意厂商原生 API；[模型配置](<../server/src/model/configuration.rs>) |
| Chat/Responses/Anthropic | HTTP 请求、SSE、工具调用、usage、reasoning replay | 需上游按协议返回；不自动兼容非流式/任意音视频/embedding/图像生成 API；[三个 provider](<../server/src/provider/mod.rs>) |
| Agent/Ask/Plan/Debug/Multitask | 各模式模板和工具子集 | 不是 Cursor 全部产品功能；Debug 使用客户端配置，不自建完整调试环境；[mode 编译](<../server/src/cursor/compile/run.rs#L562-L574>) |
| 文件/Shell/MCP/Task | 服务组装协议并让 Cursor 执行，再处理结果 | 不脱离 Cursor 独立拥有完整 IDE 执行环境；[派发](<../server/src/cursor/tools/tool_call_dispatch/mod.rs#L43-L68>) |
| 会话/checkpoint/恢复 | canonical messages、CAS blobs、工具待完成契约、SQLite | 无工具副作用 exactly-once 保证；恢复会重派未完成工具，不是进程现场恢复；[恢复](<../server/src/cursor/checkpoint/recovery.rs>) |
| 取消/运行中续聊/后台完成 | 取消模型和执行、generation 交替、继续当前 Run、后台消息 | 新上下文更新与 Resume 有实际遗漏，见下节；[协调器](<../server/src/cursor/conversation/runtime.rs>) |
| 压缩 | 手动/自动压缩、context overflow 后压缩重试 | 前缀在压缩时重置；未知 context window/超大当前输入仍可能失败；[压缩](<../server/src/run/compaction.rs>) |
| 模型目录/账号 | 合并本地模型、部分 mock、官方响应 patch/fallback | 模型列表仍先请求官方；mock Ultra 不等于官方权益；[目录](<../server/src/cursor/services/model_catalog.rs#L257-L313>) |
| 登录 | 若没有既有非空官方 token，会注入本地 JWT；已有官方 token 保留 | 本地 Agent 不应概括为必须官方登录；官方功能仍需有效账号/额度；[账号注入](<../server/src/local_app/account.rs#L41-L84>) |
| TAB | 16 个 RPC 透传公共/官方/自定义服务 | 不是本地 TAB 后端；自托管 TAB 实现只链接到旧 archive，不在当前 workspace；TAB（`git show 6a13780:server/src/cursor/services/tab.rs`） |
| Git Commit message | 可用配置模型本地生成 | 默认跟随 TAB 目标，diff 可能发作者服务；Commit（`git show 6a13780:server/src/cursor/services/commit_message.rs`，历史 46–67 行） |
| WebSearch/WebFetch | 23 源聚合搜索、HTTP 页面提取 | 不是官方完整搜索后端，无浏览器 JS 渲染，来源可反爬/失效；[搜索](<../server/src/search/federation.rs>)、[抓取](<../server/src/search/fetch.rs>) |
| Semble | 本地代码 embedding/index/search | 首次下载模型，远程 repo 会 git clone；不替代 Cursor 官方全量索引服务；[Semble 适配](<../server/src/search/search_provider.rs>) |
| 协议/构建材料 | workspace、prompt、migrations、两套 lock、CI/release 都存在 | 只编译 agent_v1；aiserver 重名导致手写必要 wire 子集；[build](<../server/build.rs#L28-L53>) |
| Docker 部署 | 前端/服务/运行镜像三阶段、非 root 用户 | 服务构建只复制部分 workspace，缺 Semble crate/benchmark manifest 路径；静态判断构建材料不足，未实际 docker build；[Dockerfile](<../Dockerfile#L13-L28>)、[workspace 成员](<../Cargo.toml#L1-L7>) |
| 辅助 Go 抓包工具 | 源文件、UI、SQLite 捕获实现 | import 的 `gen/agent/v1` 和 `gen/aiserver/v1` 未包含；需 protoc/protoc-gen-go 生成，抽取协议重名问题仍需验证；[imports](<../support/cursor-capture/decode.go#L14-L16>)、[生成脚本](<../support/cursor-protocol-extractor/scripts/generate.sh>) |

### 4.1 已确认的功能/正确性缺口

1. **运行中续聊没有完整追加新的 request context。** active UserMessage/InjectContext 只编译 runtime 消息，不按新 Run 路径投影 rules/skills/environment/MCP 或重新配置 dynamic_tools。用户运行中改变这些上下文时，新内容可能不送进下一次模型调用。[运行中处理](<../server/src/cursor/conversation/output.rs#L846-L865>)、[注入](<../server/src/cursor/conversation/output.rs#L903-L964>)、[运行消息](<../server/src/cursor/compile/break_messages.rs#L25-L68>)。
2. **ResumeAction.request_context 没有消费。** context 提取只读 dynamic_context、UserMessageAction、ExecutePlanAction；Resume 落入无新事件的路径，恢复时提供的新 MCP/terminal/admin denylist 可能缺失。[context 提取](<../server/src/cursor/compile/context.rs#L180-L194>)、[action fallback](<../server/src/cursor/compile/run.rs#L501-L510>)。
3. **子代理多个 override 只用第一项。** 不是按 subagent_type 选择；第一项 Disabled 或固定模型可能覆盖其他类型 Task。[override](<../server/src/cursor/compile/run.rs#L147-L164>)、[exec context](<../server/src/cursor/compile/run.rs#L589-L604>)。
4. **多种 ConversationAction 未实现，运行中只 warn 后忽略。** 包括 ShellCommand/StartPlan/AsyncAskQuestionCompletion/BackgroundShell/BackgroundSubagent/SubscriptionNotification/GoalContinuation；这不是只凭 TODO 推断，当前测试也固定忽略行为。协议声明不等于支持，无反馈会让相应 UI 动作失效。[实际分支](<../server/src/cursor/conversation/runtime.rs#L397-L490>)、[测试](<../server/tests/interrupt.rs#L1361-L1418>)。
5. **`thinking_budget_tokens` 保存但没有进入内建 Anthropic 请求。** Provider 仅设置 adaptive thinking 和 output_config effort；预算设置不会产生预期效果，旧版 Messages 兼容服务也可能不支持这些字段。[配置字段](<../server/src/model/configuration.rs#L122-L150>)、[Provider](<../server/src/provider/anthropic.rs#L303-L315>)。
6. **图像生成功能未完成。** GenerateImage 批准后要求不存在的配置 executor；正常编译中 supports_image_generation 为 false，工具被移除。因此属于休眠能力，不应描述成已提供通用图像生成。[错误分支](<../server/src/cursor/tools/tool_call_result/interaction.rs#L112-L115>)、[工具过滤](<../server/src/cursor/prompting/compiler.rs#L36-L38>)。
7. **模型“测试通过”不验证 Agent 工具能力。** 连接测试发送数字序列提示、tools 为空；只能验证部分请求/流式/延迟。弱 function calling 模型仍会在 Cursor Agent 中失败。连接测试（`git show 6a13780:server/src/control/service.rs`，历史 419–459 行）。

### 4.2 提示词缓存/前缀稳定的边界

有 request-context 身份、内容去重、checkpoint encode/decode、通用历史投影等实现，但不能声称所有 provider 请求永远保持严格结构前缀：

- 去重比较请求带来的 `base_messages`；无 conversation_state 的后续 generation 与已有数据库 head 并不等价，相同 context 也可能再次追加。[历史来源](<../server/src/cursor/compile/run.rs#L80-L87>)、[去重调用](<../server/src/cursor/compile/run.rs#L225-L238>)、[后续请求清 state](<../server/src/cursor/conversation/runtime.rs#L420-L423>)。
- assistant 结尾时临时添加 continue user 消息，不持久化；下一轮此前模型见过的这个尾消息可能消失。[Run](<../server/src/run/engine.rs#L239-L250>)、[临时历史](<../server/src/run/history.rs#L5-L28>)。
- Anthropic 每次重做最新 user 的 cache_control、合并同角色邻接消息，不能把 canonical 前缀等同 wire JSON 前缀。[cache](<../server/src/provider/anthropic.rs#L403-L424>)、[合并](<../server/src/provider/anthropic.rs#L447-L457>)。
- compiled context 为空时不投影新事件。若该输入表示撤销旧上下文，就没有显式撤销消息；“空表示撤销还是保持”还需根据 Cursor 实际协议验证。[空 context](<../server/src/cursor/compile/break_messages.rs#L228-L260>)。

这些是正确性/缓存稳定问题；**未测试实际服务商缓存计费，不能推断每次必然缓存失效或费用增加多少**。现有 prefix 测试主要覆盖局部投影，并非所有二轮真实请求、动态 context 和三个 provider 的完整 wire 前缀断言。

## 5. 安全问题与证据

严重度表示处理顺序，不是已复现 CVSS。下面明确区分已确认数据流、条件性风险和未做利用验证的项目。

### S1 — 高：公共 TAB/默认 Commit 透传工作区内容及请求凭据（历史；原默认路径已移除）

- 默认设置（`git show 6a13780:server/src/store/settings.rs`，历史 69–112 行） 为 Public，地址是作者公共服务。
- TAB 清单（`git show 6a13780:server/src/cursor/services/tab.rs`，历史 12–28 行） 包含 StreamCpp、RefreshTabContext、CppAppend、FSUploadFile/FSSyncFile、代码指标等。
- Commit 直连（`git show 6a13780:server/src/cursor/services/commit_message.rs`，历史 58–67 行） 复用 TAB service_url，所以 default Public 下 diff/前序 commit 等原始 RPC 同样去公共服务。
- [转发实现](<../server/src/api/cursor/proxy.rs#L76-L104>) 只删除 upstream URL/Host/hop-by-hop headers，**没有过滤 Authorization、Cookie、Cursor 账号头**，并原样传 body。

**影响：** 作者服务器会得到该 RPC 的原始 payload；当请求包含官方账号 token 时，也会得到 token。文件具体内容/头是否每次发送取决于 Cursor 客户端版本与该请求，本次没有抓包，不断言“所有文件自动全量上传”。也没有证据证明公共服务器滥用了收到的数据。

**建议：** 敏感代码不要使用默认 Public；可改官方 Direct（仍外联官方），或审查后自托管 TAB。Commit 明确选本地模型。第三方转发必须重建最小 headers，不能继承官方凭据；若曾使用主账号，应结合流量与账号操作评估 token 撤销/轮换。

### S2 — 高：管理 API 无鉴权，能够读写模型 Key、OAuth 和设置

管理路由（`git show 6a13780:server/src/control/mod.rs`，历史 112–240 行） 只有 CORS，没有 session/token/Host 校验；[模型列表](<../server/src/control/models.rs#L29-L31>) 直接返回含 api_key 的 ModelConfig；[模型对象](<../server/src/model/configuration.rs#L127-L151>) 可序列化。

- CORS（`git show 6a13780:server/src/control/mod.rs`，历史 243–284 行） 除 localhost/Tauri，还允许任意私网/链路本地 IP Origin。CORS 不是鉴权；其他本地进程不受浏览器 CORS 限制。浏览器的 private-network 限制可能降低部分网页利用，但不能作为服务鉴权。
- [默认监听](<../server/src/config.rs#L71-L75>) 是 loopback，桌面也是 loopback，这是有效降低暴露面的措施；但 `CURSOR_LISTEN_ADDR` 可以设非 loopback，管理和推理 API 共用同一 listener，组装（`git show 6a13780:server/src/app.rs`，历史 55–91 行）。Docker 暴露也必须重新审查。
- 可以读取 Key、改模型目标、启停接管、清数据、操作 OAuth 资源。管理路由没有任何 key 保护。

**建议：** 服务端管理鉴权、严格 Host/Origin/CSRF 边界、默认只 loopback，敏感值默认不出列表。修复前不要将端口暴露网络，也不要把不可信本地网页作为有权限的管理 origin。

### S3 — 高：OAuth 批量导出直接返回所有秘密

[export_resources](<../server/src/plugin/registry.rs#L789-L803>) 原样导出 records.private_data；[HTTP 导出](<../server/src/control/plugins.rs#L60-L77>) 返回附件 JSON，未设置 `Cache-Control: no-store`。账号 access/refresh token 等会包含在导出内容中。

桌面[导出操作](<../apps/desktop/src/features/plugins/PluginManagementPage.tsx#L184-L197>) 把本地导出 URL 交系统浏览器；普通资源卡片的安全 view 不抵消这个未鉴权导出接口。

**建议：** 合并 S2 修复；导出明确二次确认、禁缓存、提供加密备份，不把可调用的未鉴权明文导出当作安全资源视图。不要公开或云同步已有导出文件。

### S4 — 高：Google 插件模型调用时刷新 token，可把 OAuth 秘密写入详细日志

已确认源码链：

```text
PluginRegistry 将 CallRecorder 传给 provider invoke
  → Antigravity 模型调用检测 token 临期
  → refreshAccount → POST oauth2.googleapis.com/token
       body: refresh_token + client_secret + client_id
  → HostContext 为 invocation 第一次网络请求认领 recorder
       只抹除敏感 headers，不抹除 body
  → token 响应 access_token/refresh_token 作为 response_chunk
  → 详细模式下落盘 → 未鉴权 call detail 可读取
```

证据：[传递 recorder](<../server/src/plugin/registry.rs#L238-L272>)、[自动刷新](<../server/plugins/build-in/antigravity-auth/provider.ts#L628-L639>)、[刷新 body/响应](<../server/plugins/build-in/antigravity-auth/resources.ts#L374-L420>)、[请求记录](<../server/src/plugin/worker.rs#L421-L440>)、[认领/记录](<../server/src/plugin/worker.rs#L517-L565>)、[详细落盘](<../server/src/store/llm_calls.rs#L101-L129>)、详情返回（`git show 6a13780:server/src/control/service.rs`，历史 629–642 行）。

**触发条件：** 配置 Google 插件账号、模型调用发生自动刷新、详细日志开启。初始迁移的[详细日志默认 false](<../server/migrations/0001_initial.sql#L176-L181>) 降低默认泄漏概率，但用户可开启，已有日志不会因关开关自动消失。登录或普通手动刷新不必然走同一 recorder 链。

**建议：** OAuth 请求/响应默认绝不进入模型日志；仅真正模型请求认领 recorder；对 body 和 URL 也做秘密过滤。若曾在详细模式触发，审查历史日志/备份，并评估撤销 OAuth 授权，清理需单独授权。

### S5 — 高：插件删除允许 `.` / `..`，存在目录越界删除

[delete handler](<../server/src/control/plugins.rs#L22-L27>) → [registry.remove](<../server/src/plugin/registry.rs#L960-L964>) → state.clear → [data.clear](<../server/src/plugin/data.rs#L114-L125>)。[validate_component](<../server/src/plugin/data.rs#L194-L203>) 允许点号且不排除单独 `.`/`..`，没有先确认已知插件。

```text
root = ~/.cursor-byok-pure/plugins/data
plugin_id = ".." → root.join("..") → plugins/data/.. → 整个 plugins 目录
plugin_id = "."  → root.join(".")  → 全部插件 data
```

**证据级别：** 删除函数的路径越界可由源码确认；编码 path segment 是否能经 HTTP 客户端/代理/Axum 原样到达，是未实测条件。**本次没有发送任何删除请求，不提供需在真实数据目录执行的复现命令。** 影响是插件安装/运行时/账号资源被误删，不是任意路径遍历无限上溯，因为字符集仍禁止斜杠。

**建议：** 插件 ID 使用 manifest 严格语法，排除 dot components；操作已知 entry；解析后的目标必须属于预期的单个子目录；破坏性接口同样需要 S2 鉴权。

### S6 — 中高：广告图片 URL 缺少私网/重定向边界（历史；广告入口已删除）

广告 URL 校验（`git show 6a13780:server/src/control/ads.rs`，历史 109–117 行） 与 validate_http_url（`git show 6a13780:server/src/control/ads.rs`，历史 253–261 行） 只确认 HTTP(S)+host；图片下载（`git show 6a13780:server/src/control/ads.rs`，历史 165–205 行） 使用默认网络 client，未执行私网地址检查，也没有显式禁重定向。

广告服务器若返回 loopback/私网图片 URL，服务端就会尝试访问。它并非普通不可信 Cursor tool 直接指定目标，而是**对广告服务被入侵/恶意返回的信任边界不足**。响应须通过图片 MIME/10MiB 校验才能缓存，但请求在此前已发出，仍有 SSRF/内部 GET 副作用风险。下载失败保留远程 image URL，桌面还可能直接加载它。

对比：[WebFetch](<../server/src/search/fetch.rs#L104-L153>) 已做 DNS/IP 检查、显式 redirect 逐跳处理和解析地址 pinning，说明不能把整个项目所有 HTTP 都概括为“毫无 SSRF 防护”。但它对域名解析的 `198.18.0.0/15` 有代理兼容例外，[例外](<../server/src/search/fetch.rs#L289-L297>)，严格隔离场景也应复核。

**建议：** 移除非必要广告或让图片使用固定可信 origin；统一公网目标校验并限制每次 redirect；失败不退回任意远程 URL。

### S7 — 中高：模型与 OAuth 凭据明文存储，脱敏仅覆盖部分位置

- SQLite [model_configs](<../server/migrations/0004_flatten_model_configuration.sql#L3-L20>) 中 api_key 和自定义 headers 是明文；配置/消息/工具结果/图片/checkpoint 也落盘。
- 插件 JSON [private_data](<../server/src/plugin/data.rs#L80-L84>) 存 access/refresh token，Unix 目录 700/文件 600；Windows 没有专用 DPAPI/显式 ACL 实现，权限设置失败部分被忽略。[写入与权限](<../server/src/plugin/data.rs#L145-L224>)。
- 常规模型 recorder [header 脱敏](<../server/src/provider/recorder.rs#L20-L37>) 是正向防护，但只过滤[六类 header 名](<../server/src/model/configuration.rs#L362-L366>)。自定义 `x-token` 等、URL query/userinfo 或额外 body 中的秘密仍可能被记录。模型 URL 允许 HTTP，也不禁止 URL credentials/query。[URL 验证](<../server/src/model/configuration.rs#L306-L320>)。
- Unix 默认数据根 700、CA key 600 降低其他 OS 用户读取风险，但不能防同用户程序、未加密备份、误同步、管理 API 读取。[数据目录](<../server/src/config.rs#L16-L23>)、[CA key](<../server/src/local_app/ca/mod.rs#L143-L144>)。

**建议：** 模型/OAuth 秘密使用 OS 凭据库或独立加密存储；配置返回秘密引用/掩码；不要云同步数据根。HTTPS 默认强制，只有明确本地模型例外；body/URL 统一脱敏。关详细日志不等于会话不落盘。

### S8 — 中高：插件安装目录可改、同版本内置插件不重新校验

[内置插件 materialize](<../server/src/plugin/builtin.rs#L177-L207>) 看磁盘 version 相同就保留文件，不校验源码 hash/签名；既有同版本 main.ts 修改会持续执行。插件还会获得所属账号完整 private resource，并可通过 manifest host allowlist 发送信息。

这是本地篡改/第三方插件信任风险，不证明当前内置插件正在恶意外传。Deno 有沙箱（见后文），但“允许某 host”不意味着其所有 path 都是可信或不能收集秘密。

**建议：** 内置插件逐文件完整性校验；第三方显式安装/启用/权限确认；私有资源细粒度授权。清空插件配置不是卸载安装目录。[remove](<../server/src/plugin/registry.rs#L960-L964>)。

### S9 — 中：桌面原生权限范围宽，Windows/Linux 可运行任意传入 shell 字符串

[capabilities](<../apps/desktop/src-tauri/capabilities/default.json#L5-L26>) 允许 main window 的所有 loopback 端口源调用相关原生能力；[open_terminal_with_command](<../apps/desktop/src-tauri/src/desktop.rs#L42-L81>) 在 Windows/Linux 将输入交 shell 执行，macOS 只打开 Terminal、忽略 command。

当前正常调用用途是 CA 安装指导；未找到“广告点击直接偷偷执行系统命令”的完整链。广告渲染不等于脚本执行；CSP（`git show 6a13780:apps/desktop/src-tauri/tauri.conf.json`，历史 14–18 行） 的 default-src self 有防护。风险是未来 XSS/不可信 loopback 页面进入有权限窗口后影响扩大。

**建议：** 原生接口只接受明确 operation，不接受任意 shell 文本；把有权限 URL scope 限到实际应用 origin/path；不载入不可信页面到 main Webview。

### S10 — 中：接管会改变系统信任与 Cursor 配置，退出不完整恢复原值

- 本地 CA 有约十年有效期，安装进系统信任；允许对 `*.cursor.sh` 作 TLS 代理，不只是某一个模型 URL。[CA](<../server/src/local_app/ca/mod.rs#L119-L144>)、[域名](<../server/src/local_app/proxy.rs#L132-L155>)。这是网关实现方式，不是偷偷植入。
- [代理设置](<../server/src/local_app/settings.rs#L62-L83>) 删除 http.noProxy、覆盖五项设置；关闭时删除这五项，不恢复之前的值，也不恢复 noProxy。JSONC 重新写成 pretty JSON 还会丢掉注释/原格式。
- [启用接管](<../server/src/local_app/mod.rs>) 可结束 Cursor；禁用只清设置、停止代理，不负责卸载系统根证书或清除注入的本地账号。

**建议：** 启用前备份 Cursor 设置/账号状态；明确退出、卸载与撤销系统 CA 的独立步骤；不要在未看过效果时运行接管。本文没有代用户做这些操作。

### S11 — 中：输出/插件消息无界造成内存或可用性风险

[OutputHub](<../server/src/cursor/transport/output.rs#L11-L39>) 保存所有历史帧、subscriber 使用 unbounded channel；长输出或慢 subscriber 可能持续占内存。插件宿主 stdout/stderr 行读取与 HostCall 并发也需限制，[worker](<../server/src/plugin/worker.rs#L321-L394>)。这类风险与管理面暴露/恶意插件有关，没有实测 DoS。

### S12 — 中高：Semble 工具未约束获准工作区，远程 git 继承本机权限

[Semble tool 参数](<../server/src/search/search_provider.rs#L69-L85>) 的 repo 直接交引擎；[本地 root 校验](<../crates/semble-core/src/source/security.rs#L7-L14>) 仅检查存在、是目录和 canonicalize，没有与获准 workspace root 对比。与批准后才执行的 WebSearch/WebFetch 不同，[派发入口](<../server/src/cursor/tools/tool_call_dispatch/mod.rs#L63-L67>) 对 Semble 直接在服务端执行。

- 模型可指定本机其他目录，搜索结果再返回调用模型。Config/All 范围可能包含有秘密的 JSON/JSON5/YAML/TOML；不只读当前工作区。实际返回内容取决于 content scope、query、忽略规则等，本次没用真实秘密验证。
- [文件发现](<../crates/semble-core/src/source/local.rs#L51-L68>) 尊重 gitignore/.sembleignore、不跟随 symlink，且排除 .git/node_modules/target 等，是真实防护；但包含隐藏文件、不做秘密内容过滤。普通无扩展名 `.env` 不在[语言 catalog](<../crates/semble-core/src/language/catalog.rs#L7-L61>) 内，**不能据此声称所有 .env 都会被索引**。
- [远程 git](<../crates/semble-core/src/source/remote_git.rs#L19-L41>) 接受 HTTP/HTTPS 任意 host、userinfo、私网和端口；用参数数组调用 git，并非 shell 字符串注入。但继承系统/用户 git 配置、env、credential helpers、proxy 等，无明确网络 allowlist/整体 timeout，不能当“受限 HTTPS 下载”。这是选择远程 repo 参数时的外联，不是每次启动自动 clone。

**建议：** 获准 workspace root 与工具读取权限绑定；明确配置文件/secret 排除；远程 repo 单独同意、HTTPS+公网校验；隔离 git 环境与凭据助手、禁不必要 hooks/config，设置超时和输出大小。

### S13 — 中：Semble 缓存信任、下载大小与供应链边界需补强

- [缓存加载](<../crates/semble-core/src/cache/store.rs#L36-L50>) 一次读入 bincode、未显式解码大小限额；[snapshot/runtime validate](<../crates/semble-core/src/index/snapshot.rs#L49-L101>) 主要检查数量/向量/索引，不验证 chunk.file_path 是安全相对路径。读取 snippets 时直接 [root.join(chunk.file_path)](<../crates/semble-core/src/search/engine.rs#L491-L503>)，没有再检查根归属。能写缓存的本地攻击者可能诱导根外读取/假搜索结果；这是本地 cache poisoning 条件，未构造 POC，不断言远程 repo 可直接写恶意缓存。
- 缓存可能包含源码片段/路径/向量；[写入](<../crates/semble-core/src/cache/store.rs#L53-L71>) 无专用加密/权限策略，安全依赖父数据目录和 OS 权限。不要把“本地 embedding”当“返回给远端模型的 snippets 没有秘密”。
- HF 资产固定 hash 是正向防护，但 [response.bytes](<../crates/semble-core/src/embedding/assets.rs#L61-L74>) 没有独立流式大小上限/完整请求 timeout；巨大响应/阻塞可能耗内存或卡初始化。
- [CI/release](<../.github/workflows/release.yml>) 使用浮动版本 Actions，签名 key 和构建处于同 job；[Dockerfile](<../Dockerfile#L3-L34>) 基础镜像未 pin digest。未发现直接上传签名秘密的脚本，但构建供应链不能仅凭 lock/checksum 宣称无风险。

**建议：** 校验所有缓存路径归属、限制 decode/download 大小，保护/清理源码缓存；预置确认后的资产。Actions pin commit SHA、镜像 pin digest、最小 job 权限，构建与签名隔离，产生 SBOM 并另行依赖审查。

## 6. 全部外联类别清单

以下覆盖当前**项目自写代码可识别的运行/工具/开发外联入口**，不仅模型请求。动态 base URL、网页/广告目标、git repo、服务返回的 OAuth URL、CDN redirects 没有有限固定域名集合；包内部/OS/原 Cursor 自身流量不在“全部域名已穷举”的保证内。

### 6.1 默认和常规外联

| 类别/目标 | 触发 | 发送数据 | 停止/配置边界 | 源码 |
| --- | --- | --- | --- | --- |
| 你配置的模型 API URL（HTTP/HTTPS） | Agent、本地 Commit、连接测试、对外模型 API | API key/custom headers、prompt/history、工具、图像及参数 | 每模型 URL/key/protocol 配置；禁用调用 | [ProviderRouter](<../server/src/provider/router.rs>)、[Chat 请求](<../server/src/provider/openai_chat.rs#L75-L101>) |
| 配置服务的模型发现 endpoint | 点击“获取模型” | 配置 API key/headers | 不点击则不触发此发现 | 发现模型（`git show 6a13780:server/src/control/service.rs`） |
| `https://api2.cursor.sh` / original `*.cursor.sh` | 官方模型、未接管 RPC、账号/模型列表/Statsig 初始化等 | 原 Cursor 请求 headers/body（包括 token 和元数据） | 没有统一“完全断开官方”模式；本地目录 fallback 也会先尝试官方 | [官方转发](<../server/src/api/cursor/proxy.rs>)、[Statsig](<../server/src/cursor/services/analytics.rs#L27-L47>) |
| `https://tab.leokun.cn` 的 16 个 TAB RPC | 接管启用且 TAB=Public（默认），Cursor 触发补全/同步 | 原始 TAB context、文件上传/同步、指标及携带的授权头 | 系统设置 TAB → Direct/自托管 Custom；不是自动改成本地模型 | TAB 清单（`git show 6a13780:server/src/cursor/services/tab.rs`） |
| 同 TAB 目标的 WriteGitCommitMessage | Commit 未选本地模型（默认），生成提交信息 | diff/commit 等原始 RPC 与原 headers | Commit 选择本地模型；或 TAB Direct/Custom | Commit forward（`git show 6a13780:server/src/cursor/services/commit_message.rs`，历史 46–67 行） |
| `https://tab.leokun.cn/api/v1/ads?placement=menu` | 桌面布局挂载、聚焦/可见、语言变化；10 秒请求节流（不是每 10 秒定时） | 持久安装 UUID、OS、app version、语言、禁用广告 IDs，及网络 IP | 未发现全局禁止广告拉取设置；禁用单个广告不等于停止请求 | 广告请求（`git show 6a13780:server/src/control/service.rs`，历史 285–313 行）、自动触发（`git show 6a13780:apps/desktop/src/shell/AppLayout.tsx`，历史 98–135 行） |
| 同广告 `/api/v1/ads/{id}/dismissals` POST | 确认关闭单个广告（原因可空；留空仍发请求） | 安装 ID、OS、版本、广告 ID、reason | 不确认关闭则不发此 POST；不等于无广告 GET | 关闭反馈（`git show 6a13780:server/src/control/service.rs`，历史 316–345 行）、UI（`git show 6a13780:apps/desktop/src/shell/AppLayout.tsx`，历史 167–185 行） |
| 广告返回的 HTTP(S) image URLs | 拉取成功后下载未缓存图片；失败时 Webview 可直接读远图 | 普通 GET/客户端 IP；不是模型 key 注入请求 | 图片本地缓存减少重复下载；远程目标由广告服务决定 | cache_images（`git show 6a13780:server/src/control/ads.rs`，历史 121–175 行） |
| 广告返回 button URL | 用户点击推广按钮 | 系统浏览器访问，目标由服务返回 | HTTP(S) 限制，无固定域名 allowlist | 广告协议（`git show 6a13780:server/src/control/ads.rs`，历史 86–106 行）、[打开浏览器](<../apps/desktop/src-tauri/src/desktop.rs#L90-L107>) |
| GitHub `leookun/cursor-byok/releases/latest/download/latest.json` | 应用启动；6 小时节流的 focus/online/visibility/timer 自动检查 | updater 版本/平台相关请求元数据/IP；未见注入模型 key | 自动检查代码存在；不是自动安装 | [App](<../apps/desktop/src/App.tsx#L53-L84>)、updater 配置（`git show 6a13780:apps/desktop/src-tauri/tauri.conf.json`，历史 34–42 行） |
| 同仓库 `portable-latest.json` | Windows portable 更新检查路径 | 同上 | Windows 专用；安装动作另行触发 | portable update（`git show 6a13780:apps/desktop/src-tauri/src/update/mod.rs`，历史 25–27 行） |
| 更新 manifest 返回的包 URL/CDN | 用户选择安装更新 | 普通更新下载/IP | Tauri updater 公钥验签；仍信任发布者可提供签名程序 | 下载/安装（`git show 6a13780:apps/desktop/src-tauri/src/update/mod.rs`，历史 81–126 行） |
| GitHub `denoland/deno/releases/download/v2.9.6/deno-<target>.zip` | 用户显式初始化插件运行时 | 平台包 GET/IP | 不初始化不会下载；固定 SHA256 后才解压执行；Deno 自更新检查关闭 | [asset](<../server/src/plugin/asset.rs>)、[installation](<../server/src/plugin/installation.rs>) |
| Hugging Face `minishlab/potion-code-16M-v2/resolve/main/{model.safetensors,tokenizer.json}` | 首次 Semble 使用且本地资产缺失/hash 不符 | 普通模型文件下载；未见上传代码给 HF 的实现 | 本地缓存/hash 正确则不再下载；redirect CDN 由上游决定 | [模型资产](<../crates/semble-core/src/embedding/assets.rs#L13-L83>) |
| repo 指定远程 git host | Semble/benchmark 使用远程 repo | git clone/fetch 网络元数据；git 可能按自身配置使用凭据 | 用本地 repo 避免该网络步骤 | [remote git](<../crates/semble-core/src/source/remote_git.rs>) |
| 用户提供的网页 URL | Cursor WebFetch Approved 后 | URL/UA/语言；本服务不附加模型 key | Cursor 审批；公网/DNS/redirect 检查及代理例外 | [interaction](<../server/src/cursor/tools/tool_call_dispatch/interaction.rs#L25-L61>)、[fetch](<../server/src/search/fetch.rs>) |

广告请求上的安装 UUID 在installation_id（`git show 6a13780:server/src/store/settings.rs`，历史 281–301 行） 生成后持久保存。它可以关联同一安装的请求，不能采信“随机 ID 就不涉及隐私”的广告注释来替代实际数据分析。

### 6.2 WebSearch：一次批准会并发发送给 23 个来源

[聚合实现](<../server/src/search/federation.rs#L56-L72>) 对完整 engines 集合 `join_all`，不是先挑一个成功来源。每个来源收到搜索 query；其中可能包含模型根据工作区生成的业务名、代码片段或其他敏感文本。不能仅按“本地聚合结果”判断搜索隐私。

固定请求目标来自[搜索 catalog](<../server/src/search/catalog.rs#L42-L229>)：

```text
www.google.com/search
www.bing.com/search
search.brave.com/search
html.duckduckgo.com/html/
www.startpage.com/sp/search
search.yahoo.com/search
www.mojeek.com/search
www.qwant.com/
www.ecosia.org/search
yandex.com/search/
www.baidu.com/s
www.sogou.com/web
www.so.com/s
search.naver.com/search.naver
search.seznam.cz/
en.wikipedia.org/w/api.php
github.com/search
api.stackexchange.com/2.3/search/advanced
crates.io/api/v1/crates
registry.npmjs.org/-/v1/search
pypi.org/search/
arxiv.org/search/
api.crossref.org/works
```

每源约 12 秒 timeout，HTML/JSON 提取、最多聚合 15 结果；没有真实公网验证可靠性。建议最小化来源并明确 query 数据去向，敏感工作区不要批准泄露内部信息的搜索。

### 6.3 OAuth/provider 插件的所有固定网络入口

这些不是桌面启动即向所有账号服务发送模型请求。一般由用户添加/导入账号、OAuth 登录、同步模型、刷新额度或选择插件模型触发；但授权/导入后的模型/额度查询，以及 token 临期刷新是继发自动行为。

| 插件 | 网络入口（同一行按目的分组） | 数据与触发 | 证据 |
| --- | --- | --- | --- |
| Codex | `auth.openai.com/api/accounts/deviceauth/usercode`、`.../token`、`auth.openai.com/oauth/token`；浏览器 `auth.openai.com/codex/device` | client_id、device code、user code、code_verifier/code 兑换；设备授权流程 | [Codex OAuth](<../server/plugins/build-in/codex-auth/oauth.ts>) |
| Codex | `chatgpt.com/backend-api/codex/models?client_version=1.0.0` | Bearer/account ID；OAuth/导入后同步或手动同步 | [Codex models](<../server/plugins/build-in/codex-auth/models.ts>) |
| Codex | `chatgpt.com/backend-api/wham/usage`；`.../rate-limit-reset-credits`；`.../rate-limit-reset-credits/consume` | token/account，查额度/重置卡；消费会改变账号额度，不只是只读请求 | [Codex resources](<../server/plugins/build-in/codex-auth/resources.ts>) |
| Codex | `chatgpt.com/backend-api/codex/responses` | 用户调用 Codex 模型：token/account、prompt/tools/image 等 | [Codex provider](<../server/plugins/build-in/codex-auth/provider.ts>) |
| Grok | `auth.x.ai/oauth2/device/code`、`auth.x.ai/oauth2/token`；浏览器 verification URL 来自响应 | device/client ID；scope 含 openid/profile/email/offline_access/grok-cli:access/api:access | [Grok OAuth](<../server/plugins/build-in/grok-auth/oauth.ts>) |
| Grok | `api.x.ai/v1/language-models`，失败 fallback `api.x.ai/v1/models` | Bearer token；账号导入/授权后同步或手动同步 | [Grok models](<../server/plugins/build-in/grok-auth/models.ts>) |
| Grok | `cli-chat-proxy.grok.com/v1/billing?format=credits` | Bearer + x-xai-token-auth；手动刷新余额 | [Grok resources](<../server/plugins/build-in/grok-auth/resources.ts>) |
| Grok | `api.x.ai/v1/chat/completions` | 模型调用 prompt/tools + token | [Grok provider](<../server/plugins/build-in/grok-auth/provider.ts>) |
| Google/Antigravity | 浏览器 `accounts.google.com/o/oauth2/v2/auth`；`oauth2.googleapis.com/token`；`www.googleapis.com/oauth2/v1/userinfo?alt=json` | state/PKCE/scope/code/token；授权、token 刷新、邮箱查询 | [Google OAuth](<../server/plugins/build-in/antigravity-auth/oauth.ts>)、[client/scope](<../server/plugins/build-in/antigravity-auth/google_oauth.ts>) |
| Google/Antigravity | 下列三域的 `/v1internal:loadCodeAssist`、`/v1internal:fetchAvailableModels` | access token、project/metadata；授权/导入后自动查 project/额度/模型，或 refresh/sync | [Google resources](<../server/plugins/build-in/antigravity-auth/resources.ts#L69-L127>)、[Google models](<../server/plugins/build-in/antigravity-auth/models.ts>) |
| Google/Antigravity | 下列三域的 `/v1internal:streamGenerateContent?alt=sse` | access token、project、prompt/system/tools/image；选择插件模型调用 | [Google provider](<../server/plugins/build-in/antigravity-auth/provider.ts#L374-L394>) |

Google 多域 fallback 的顺序：

```text
daily-cloudcode-pa.googleapis.com
  → cloudcode-pa.googleapis.com
  → daily-cloudcode-pa.sandbox.googleapis.com
```

**同一 token，以及模型请求内容可能随重试进入 daily/prod/sandbox 多个端点。** 这些是 Google 域名，不等于“作者暗门”，但应视为额外数据路由，不宜未说明地用于敏感代码。建议显式选择允许的环境并控制 fallback。

Google scopes 包括 cloud-platform、userinfo.email、userinfo.profile、cclog、experimentsandconfigs，申请 offline 授权；范围较广。源码内 client secret 是公开桌面 OAuth client 的常量、分片拼接，不是用户 Key；“Official”注释不是本次已验证的官方身份或授权复用证明。应核对 client 来源/授权方式，采用最小 scope 和自己注册的 client。

OAuth callback 本机随机端口、有 state 与 PKCE、一次性完成/超时关闭，是正向防护；[回调](<../server/src/plugin/oauth_callback.rs>)、[流程](<../server/src/plugin/registry.rs#L372-L455>)。但浏览器 authorization URL 由插件提供，HTTP(S) 校验不限定授权域，第三方插件仍有钓鱼/误授权风险。

### 6.4 开发、辅助工具、README 的外联

| 类别 | 目标与触发 | 结论/证据 |
| --- | --- | --- |
| npm 安装 | 当前 lock 的 resolved host 均为 registry.npmjs.org；部分依赖 install script 可能有额外行为 | 没有执行安装；有 integrity 不等于包无恶意。hasInstallScript：@parcel/watcher、fsevents；lock（`git show 6a13780:apps/desktop/package-lock.json`） |
| Cargo 构建 | crates.io registry/index、下载包和 build scripts | lock 有 checksums；未递归源码审计依赖/公告。native build/protoc 等 build dependency 是构建链，不是运行时广告；[server Cargo](<../server/Cargo.toml>) |
| Docker/CI/release | 拉基础镜像、系统包、Rust/npm，GitHub Actions/发布 assets | 只在用户执行构建/CI 时；不是运行启动暗门；[Dockerfile](<../Dockerfile>)、[CI](<../.github/workflows/ci.yml>)、[Release](<../.github/workflows/release.yml>) |
| cursor-capture 抓包工具 | 固定 api2.cursor.sh；界面从 cdn.jsdelivr.net 加载 Monaco 0.56.0 | 启动会打开浏览器、透传并落盘；当前缺生成 Go 包，不随主应用自动启动；[代理](<../support/cursor-capture/server.go#L106-L142>)、[CDN](<../support/cursor-capture/web/index.html#L136>) |
| protocol extractor | 可选 `npx prettier` | 直接用工具且未 skip-format 时可能下载安装 formatter；默认封装脚本使用 skip-format。会替换协议输出目录，不要当无副作用检查；[extractor](<../support/cursor-protocol-extractor/extractor/main.go#L100-L119>)、[脚本](<../support/cursor-protocol-extractor/scripts/extract.sh>) |
| benchmark | case 中远程 repo、git fetch/clone、codegraph adapter/本机命令 | 仅显式运行 benchmark；不等于生产自动上传代码；[benchmark](<../support/benchmarks/semble/README.md>) |
| README 展示 | shields.io 徽章；GitHub Baocode 首行推广、docs.leokun.cn、release/issues、Telegram 等链接 | 渲染 Markdown 徽章或点击才访问；桌面运行广告另有代码实现。只删 README 首行不会移除运行广告；中文 README（`git show 6a13780:README-CN.md`，历史 1–14 行） |

浏览器打开普通链接时，会按其自身会话/cookie/网络设置发请求；这与 Rust 服务主动插入模型 Key 是不同的数据路径。测试里的 example.com/localhost 样例和 `$schema` URL 不应全部误报成生产启动请求。

## 7. 已存在的正向防护，以及它们的边界

- 服务/本地代理默认 loopback；Unix 默认数据根 700、CA key 600。**不能防同用户进程和未鉴权管理 API。**
- 官方 original URL header 要求 HTTPS 且 Cursor 域名；cursor_client 禁重定向。**公共 TAB 目标另走 service_url，不执行第三方最小化 headers。**
- WebFetch 明确检查私网/DNS/重定向/大小/时间，域名解析地址 pinning。**广告 fetch 和插件 host 并不复用完全相同约束。**
- Deno 不使用 `--allow-all`：仅允许读插件/SDK 目录，禁止直接网络/进程/env/write/远程/npm import，网络由 HostContext HTTPS+精确 host allowlist 代发，禁止 redirect。**插件可使用该 host 的任意 path/port，仍能把它持有的资源发到允许服务；本地篡改 manifest 也受安装信任影响。** [worker](<../server/src/plugin/worker.rs#L254-L283>)、[network](<../server/src/plugin/worker.rs#L467-L527>)。
- Deno ZIP 固定 SHA256、大小限制、固定 deno 文件解压、核对版本后执行。**后续启动只看 executable/ready 存在，不逐次 hash 本地二进制。** [installation](<../server/src/plugin/installation.rs>)、[runtime](<../server/src/plugin/runtime.rs>)。
- Semble 模型/tokenizer 用 safetensors/JSON，固定 SHA256 后使用。**首次仍外联 HF，未核对上游是否持续提供该 hash 内容。**
- Tauri 更新由内置公钥验证签名，检查与用户安装分开。**当前目录名虽是 pure，更新配置仍指向 `leookun/cursor-byok` 上游仓库；安装可能替换成未被本次审查的上游程序。签名只证明持 key 发布者提供，不证明新版本无恶意或等于当前源码；自有 fork 需自己的发布端点/签名 key。**
- 标准 Authorization/x-api-key/cookie 在常规模型详细记录中抹除。**OAuth body、URL 与其他自定义 secret header 没有全面覆盖。**

未发现内置插件向作者广告域发送模型 Key/OAuth token 的专门代码，也未发现项目自写代码中的挖矿/远控/远端任意脚本下载执行链。但是现有公共请求透传、更新信任和本地安全漏洞已经足以使“无任何欠缺/绝对安全”结论不成立。

## 8. 针对“纯本地、只访问自己的模型 API”的处理清单

**以下保留首次任务的历史建议，不代表全部已实施。** 当前仅公共 TAB/Commit 默认路径、广告与上游更新源的处理已落实；更新策略按用户选择为“保留 updater、无源短路”，不是删除 updater 或改成只手动。其余建议仍待独立处理。

### 第一优先级：泄漏与破坏性操作

1. 去掉公共 TAB/Commit 默认路由，明确自托管、关闭该能力或官方直连的取舍；跨目标转发清除官方凭据。
2. 管理 API 增加真实鉴权、Host/Origin/CSRF 校验，与对外模型 API 分离；列表不直接返回 key。
3. 插件删除仅允许已知严格 ID、检查目标目录归属；拒绝 `.`/`..`。
4. OAuth 请求/响应不写模型日志，secret 统一抹除；导出鉴权/确认/加密/no-store。
5. 用低权限测试 Key 和非主账号；如果曾触发高风险路径，分别评估模型 Key 轮换、Cursor token 和 Google OAuth 撤销。不要未经授权直接删除用户历史或自动撤销账号。

### 第二优先级：减少不必要外联与信任

6. 删除运行时广告拉取/图片/关闭反馈，不只是隐藏 UI 或 README 推广；统一目标检查。
7. 自动更新改显式操作；自有 fork 使用自己的端点和签名 key，而不是继续信任上游更新链。
8. 不需要的账号插件不启用；需要时明确列出 OAuth scope、额度变更、多域 fallback，验证插件和本地 Deno 完整性。
9. WebSearch 限制来源/显式 query 提示；Semble 预置已验证模型/使用本地 repo；本地 repo 文件读取仍需要单独评估工作区范围。
10. 如果要求“除选定模型 API 外零外联”，还要限制官方 passthrough、metadata 请求以及原 Cursor 自身网络。**这不是现在某个配置开关能完整实现的保证，不能承诺设置几项就离线等同完整 Cursor。** 需要明确功能损失与网络出口白名单，并做实机流量验证。

### 第三优先级：功能稳定性

11. 补齐 Resume/active context 更新、子代理 override、未支持 action 反馈；移除或实现悬空 budget/image 能力。
12. 针对两个完整用户 turn、context 相同/变化/恢复、三个 provider wire 添加结构测试，明确压缩重置边界。
13. 补工具调用能力测试，而不只是文本连接测试；控制输出与插件队列大小。
14. 具备工具链后跑现有 CI 检查，再测试明确的 Cursor 版本与真实兼容服务：多轮、工具、拒绝/取消、网络断开、重启恢复、压缩、TAB 和退出/卸载恢复。

## 9. 最终判断的使用方式

- **“核心链路实现存在且模块材料较齐全”是源码判断，不是本机编译成功。**
- **“有广告及作者公共透传”是确定事实，不需要先证明恶意意图才算重要。**
- **“有安全漏洞”与“作者故意植入后门”不是同一个命题。** 前者有明确源码链，后者本次没有充分证据。
- 不能据此给上游发布二进制、所有依赖、新版本、外部公共服务器隐私承诺或 Cursor 自身行为出具无后门保证。
- 本次仅新增本报告和[阅读指南](<./CODE-READING-CN.md>)，没有顺手修复任何源码/配置/依赖问题；后续修改需要另行确认范围。

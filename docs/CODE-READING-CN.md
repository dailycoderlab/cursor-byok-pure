# Cursor BYOK 源码阅读路径

> 依据当前下载的源码快照整理，日期：2026-11-05。阅读目标是理解“Cursor 请求如何变成自己的模型 API 请求”，不是从头顺读所有文件。安全性与完整性结论见[项目审查报告](<./PROJECT-AUDIT-CN.md>)。本文描述现有结构，不提出代码重构，也不执行配置、证书或代理修改。

## 1. 先建立正确的项目定位

**这是本地 Cursor 模型网关、Agent 协议适配器和桌面管理程序，不是 Cursor 官方后端的完整复制品。**

两个入口共用一套 Rust 服务：

```text
独立服务进程                                  桌面进程
server/src/bin/cursor-server.rs                apps/desktop/src-tauri/src/main.rs
  Config::from_env()                             lib::run() → desktop::run()
           │                                            │ Config::desktop()
           └────────────────┬───────────────────────────┘
                            ▼
                      server/src/app.rs
                  App::new → bind → serve_on
                            │
              ┌─────────────┼─────────────────┐
              ▼             ▼                 ▼
        Cursor 协议 API   本地管理 API      对外兼容模型 API
        BidiAppend/RunSSE /__byok-api__/api  /byok/v1/*
              │             │                 │
              │         配置/数据库             │
              └─────────────┴─────────────────┘
                            ▼
                 ProviderRouter → 你配置的 API
```

还存在 Cursor 官方转发、公共 TAB、广告、更新、搜索、模型文件下载和 OAuth 插件这些旁路。先读懂核心网关，再逐一审查旁路，不能用“本地运行”推导“不会外联”。

## 2. 当前目录就是架构

以下是现有目录；不是拟议改造。行数为本次实测物理行数，**包括空行、注释和文件内测试**，未计算构建目录；目录的职责和文件的职责分别标明。

```text
cursor-byok-pure/
├── Cargo.toml                              # Rust workspace 成员和构建 profile
├── Makefile                                # 开发/检查/构建命令入口
├── apps/desktop/                           # React 管理页面与 Tauri 桌面进程
│   ├── src/
│   │   ├── index.tsx                       # React 挂载入口，15 lines
│   │   ├── App.tsx                         # 页面路由与自动检查更新，87 lines
│   │   ├── features/                       # models/settings/plugins/calls/home 业务页面
│   │   ├── shell/                          # 应用窗口布局与广告 UI
│   │   ├── shared/api.ts                   # 管理 API 类型及 HTTP 客户端，563 lines
│   │   ├── shared/store/                   # 页面全局状态及更新状态
│   │   ├── shared/native/                  # 桌面原生能力调用
│   │   ├── shared/ui/                      # UI 组件，不承担模型推理
│   │   └── i18n/                          # 本地化运行时与语言文本
│   └── src-tauri/
│       ├── src/desktop.rs                  # 服务/窗口/退出生命周期，320 lines
│       ├── src/startup.rs                  # 启动诊断与日志
│       ├── src/update/                     # 更新检查、签名验证与程序替换
│       └── capabilities/default.json       # Webview 的原生能力权限边界
├── server/
│   ├── src/
│   │   ├── bin/cursor-server.rs            # 独立服务器 main，16 lines
│   │   ├── config.rs                       # 进程配置/数据目录/监听地址，170 lines
│   │   ├── app.rs                          # 依赖组装与服务启停，208 lines
│   │   ├── api/
│   │   │   ├── cursor/handlers.rs          # RPC 路由及本地/官方分流，392 lines
│   │   │   ├── cursor/bidi.rs              # 上行 BidiAppend 解码与入队，211 lines
│   │   │   ├── cursor/run_sse.rs           # 下行事件流与断开处理，232 lines
│   │   │   ├── cursor/proxy.rs             # 官方 Cursor 请求转发
│   │   │   └── byok/                       # OpenAI/Anthropic 对外 HTTP 入口
│   │   ├── local_app/                      # Cursor 本机接管：CA、代理、设置、账号、进程
│   │   ├── cursor/                         # Cursor 特有语义，不能混入通用 Run 层
│   │   │   ├── protocol/                   # Connect/protobuf 编解码
│   │   │   ├── transport/                  # request_id、上行序号与下行输出生命周期
│   │   │   ├── conversation/               # 协调当前 Run、后续消息、工具、取消
│   │   │   ├── compile/                    # Cursor 请求编译成通用 PreparedRun
│   │   │   ├── prompting/                  # 读取 prompt/mode/tool 配置并组装提示词
│   │   │   ├── checkpoint/                 # Cursor ConversationState 与本地历史互转
│   │   │   ├── tools/                      # 工具派发、审批、客户端执行结果与重放
│   │   │   └── services/                   # 模型目录、账号、TAB、知识、指标等 RPC
│   │   ├── run/                            # 与 Cursor/provider 无关的 Agent 循环
│   │   │   ├── engine.rs                   # 推理→工具→继续/压缩/终止，1066 lines
│   │   │   ├── port.rs                     # Run 与适配器之间的 command/event 通道
│   │   │   ├── model_cycle.rs              # 单次模型事件流归并
│   │   │   ├── tool_round.rs               # 等待/提交工具结果，230 lines
│   │   │   ├── history.rs                  # assistant 结尾时添加临时 user 消息，60 lines
│   │   │   └── compaction.rs               # 自动/手动历史压缩，508 lines
│   │   ├── model/                          # 通用领域数据结构
│   │   │   └── projection.rs               # CanonicalMessage → provider 历史，219 lines
│   │   ├── provider/                       # 三种模型协议及插件模型的序列化与事件归一化
│   │   │   ├── router.rs                   # 配置选择、HTTP client、记录器，421 lines
│   │   │   ├── openai_chat.rs              # Chat Completions 请求/SSE
│   │   │   ├── openai_responses.rs         # Responses 请求/SSE/reasoning replay
│   │   │   ├── anthropic.rs                # Messages 请求/SSE/thinking/cache
│   │   │   └── recorder.rs                 # 调用耗时、token、请求/响应记录
│   │   ├── store/                          # SQLite Repository 与写入协调
│   │   │   └── sqlite.rs                   # 建连接池、WAL、外键、迁移，49 lines
│   │   ├── control/                        # 模型/设置/插件/记录/接管/广告管理 HTTP API
│   │   ├── network.rs                      # 网络代理配置与可复用 outbound clients
│   │   ├── plugin/                         # Deno 插件发现、沙箱、OAuth、资源与网络宿主
│   │   └── search/                         # 网页搜索/抓取以及 Semble 调用适配
│   ├── prompt/cursor/                      # 各模式 prompt/runtime、工具 schema、commit prompt
│   ├── migrations/                         # SQLite 0001–0009，现有 schema 由完整序列得到
│   ├── plugins/build-in/                   # codex-auth / antigravity-auth / grok-auth 实现
│   └── tests/                              # 用 FakeCursor/FakeProvider 验证生命周期
├── crates/semble-core/                      # 本地代码切分、embedding、索引与搜索
├── protocols/cursor/                        # 抽取的 protobuf 定义，不等于 RPC 都已实现
└── support/                                # 非主应用：协议提取、流量抓包、搜索 benchmark
```

现有说明中使用的 `server/src/harness`、`cursor/request` 等名称，不是当前快照的真实目录：对应阅读入口是 `local_app` 和 `cursor/compile`。不要沿旧名称寻找不存在的模块。

## 3. 最短主线：先读这 12 个入口

不需要第一遍读完每个大文件。先在下面的文件中找指定函数，再跟它的调用关系。

| 顺序 | 文件 | 第一遍重点 | 读完应能回答 |
| --- | --- | --- | --- |
| 1 | [独立入口](<../server/src/bin/cursor-server.rs>) | `main` | 谁加载配置、启动服务？ |
| 2 | [配置](<../server/src/config.rs>) | `from_env`、`desktop`、`managed_data_dir` | 端口和数据库在哪里确定？ |
| 3 | [应用组装](<../server/src/app.rs>) | `App::new`、`serve_on` | Provider、Store、Registry、Control 谁共享？ |
| 4 | [Cursor 路由](<../server/src/api/cursor/handlers.rs>) | `bidi_handler`、`run_sse_handler` | 哪个模型走本地，哪个走官方？ |
| 5 | [Bidi 解码](<../server/src/api/cursor/bidi.rs>) | `decode`、`append` | protobuf 怎样变成内部上行消息？ |
| 6 | [Transport 注册表](<../server/src/cursor/transport/registry.rs>) | `get_or_create_for_append` | request_id 怎样对应 actor 和输出流？ |
| 7 | [Conversation 协调器](<../server/src/cursor/conversation/runtime.rs>) | `spawn`、`start_generation` | 同一连接里的输入、工具结果和 Run 如何协调？ |
| 8 | [请求编译](<../server/src/cursor/compile/run.rs>) | `prepare`、`action` | 上下文、历史、模式、模型怎样成为 PreparedRun？ |
| 9 | [Run 循环](<../server/src/run/engine.rs>) | 从 `self.provider.stream` 向前后追踪 | 为什么一次用户输入会调用模型多次？ |
| 10 | [历史投影](<../server/src/model/projection.rs>) | `project_messages`、`project_tool_round` | 存储消息怎样变成模型可消费的历史？ |
| 11 | [Provider 路由](<../server/src/provider/router.rs>) | `Provider::stream` | UI model_hash 怎样换成上游 model_id、URL、key？ |
| 12 | [OpenAI Chat 适配器](<../server/src/provider/openai_chat.rs>) | `stream`、`openai_chat_messages` | 最终 HTTP 请求是什么，SSE 怎样拆成事件？ |

之后比较 [Responses 适配器](<../server/src/provider/openai_responses.rs>) 与 [Anthropic 适配器](<../server/src/provider/anthropic.rs>)，只比较“请求结构、工具结构、reasoning 回放、usage、终止判定”，不要同时展开三个完整文件。

## 4. 把一次 Agent 请求沿上下行画完整

Cursor 不是把所有数据一次 POST 给 `/v1/chat/completions`。它使用两个方向的协议通道。

```text
Cursor 客户端（UI、工作区、工具执行、审批）
    │
    │ HTTPS 请求；本地 CA 用于解密允许的 *.cursor.sh
    ▼
local_app/proxy.rs：本机 HTTP(S) 代理
    ├─ 接管清单之外的路径 ──────────────────→ 原服务（仍可能访问官方）
    └─ 接管路径 → 改为本地 backend URL，保留 original URL header
                         │
                         ▼
api/cursor/handlers.rs
    │
    ├─ BidiAppend（上行：启动请求、工具结果、交互/取消/追加消息）
    │     → Connect 解码 → bidi::decode
    │     → 检查 model_hash / plugin: 前缀
    │        ├─ 不是本地模型 → CursorProxy 转发官方
    │        └─ 本地模型 → OrderedInbox 按 append_seqno 入队
    │                         ↓
    │               ConversationRuntime（持有当前 RunGeneration）
    │                         ↓
    │               compile::prepare → Store/checkpoint/context
    │                         ↓
    │                     RunEngine
    │                         ↓
    │               ProviderRouter → 配置 API → SSE → ModelEvent
    │                         ↓
    │               RunEvent / 工具调用 / 新 checkpoint
    │                         ↓
    │               ConversationOutput → AgentServerMessage → OutputHub
    │
    └─ RunSSE（下行：订阅同一 request_id 的 OutputHub）
          ← SSE/Connect 输出、工具请求、checkpoint、结束事件
                         │
                         ▼
                  Cursor 执行工具/显示输出
                         │
                         └── 执行结果再次 BidiAppend，上面的循环继续
```

相关细节：

- [代理接管路径](<../server/src/local_app/proxy.rs#L109-L196>)：只拦截 Cursor 域名；非本地路径仍由代理转发，不应把这看作断网隔离。
- [本地/官方分流](<../server/src/api/cursor/handlers.rs#L228-L301>)：依据配置模型标识，不是任意一个 Cursor 模型名称都能直接走自定义 API。
- [上行顺序](<../server/src/cursor/transport/inbox.rs>)：处理序号、重复或乱序，先读它的测试再看内部细节。
- [下行订阅和关闭](<../server/src/api/cursor/run_sse.rs>)、[输出聚合](<../server/src/cursor/transport/output.rs>)：网络流断开属于 transport 行为，不能简单等同于 conversation 历史被删掉。

## 5. 三种 ID 与生命周期必须分开

| 概念 | 谁管理 | 存在哪里 | 用途 |
| --- | --- | --- | --- |
| `request_id` | TransportRegistry | 运行时映射；关联 trace/run | 一条 Bidi/RunSSE 传输的身份 |
| `conversation_id` | Conversation/Store | conversations/messages/checkpoints | 跨轮会话与持久历史的身份 |
| `run_id` | compile/RunEngine | runs/tool_rounds/llm_calls | 某次实际执行的身份 |
| checkpoint_id | CheckpointBuilder/Store | conversation_checkpoints/checkpoint_messages | 某个历史快照，不是完整复制一份 UI 状态 |
| provider_call_index | RunEngine/Store | runs、llm_calls | 同一次 Run 中第几次模型调用 |
| exec_id / call_id | 工具 runtime / 模型 | pending 工具、tool_round_calls | 客户端执行与模型工具调用的对应关系 |

同一 request_id 可以承载多次 Run，不要用它替代 run_id。下一条用户消息可能加入当前正在运行的 Run，也可能在完成后创建新 RunGeneration。

```text
开始前             执行中                         一轮完成后
无 current         current = RunGeneration N      waiting_for_action = true
    │                 │                              │
    │ AgentRunRequest │ 新 UserMessage/Inject         ├─ 后续 UserMessage → 新 Generation
    └─→ compile/Run ──┤ → active Run 的消息/中断通道   └─ 约 2s 未收到 action → drain/close
                      │
                      ├─ 工具结果 → 现有工具轮次 → 继续同一 Run
                      ├─ Background 完成 → 插入通知/唤醒
                      └─ Cancel/Disconnect → abort 工具，cancel Run，完成 transport
```

请沿 [Conversation 协调器](<../server/src/cursor/conversation/runtime.rs>) 与 [RunHandle](<../server/src/run/handle.rs>) 阅读，而不是把所有后续消息都理解成“重新启动一个会话”。

## 6. 第二遍阅读主题

### 6.1 配置如何变成请求

```text
CursorModelEditor.tsx
  → shared/api.ts
  → control/models.rs + ControlService
  → model/configuration.rs（校验、URL 拼接、model_hash）
  → store/models.rs（SQLite model_configs）
  → Cursor 的 model_catalog RPC（模型列表）
  → 用户在 Cursor 选择本地 model_hash
  → ProviderRouter（查配置、替换真实 model_id、URL、headers/key）
```

按 [模型编辑器](<../apps/desktop/src/features/models/CursorModelEditor.tsx>) → [管理客户端](<../apps/desktop/src/shared/api.ts>) → [模型校验](<../server/src/model/configuration.rs>) → [模型 Repository](<../server/src/store/models.rs>) → [Cursor 模型目录](<../server/src/cursor/services/model_catalog.rs>) 阅读。

关键问题：`display_name` 是 UI 名称；`model_hash` 是本地选项标识；`model_id` 才是上游 API 接受的名称。完整 URL 与基础 URL 的追加逻辑也在模型配置层，而不在 React 页面或 Cursor 协议层。

### 6.2 提示词、上下文与历史

```text
Cursor RequestContext / selected_context / blobs
  → context::hydrate + 本地 rules
  → PromptCompiler + prompt/cursor/{mode}
  → request-context:<event>（规则/skills/env/MCP 等）
  → runtime:<event>（本轮 query/选中文件/时间/动作）
  → CanonicalMessage → checkpoint → project_messages
  → provider 序列化
```

依次读 [PromptCompiler](<../server/src/cursor/prompting/compiler.rs>)、[上下文编译](<../server/src/cursor/compile/context.rs>)、[运行消息编译](<../server/src/cursor/compile/break_messages.rs>)、[消息定义](<../server/src/model/message.rs>)、[历史投影](<../server/src/model/projection.rs>)。

不要把“设计上尽量追加历史”当成“所有场景 provider 请求严格字节前缀不变”。现有临时 continue 消息、Anthropic 合并/cache 标记，以及 context 去重/动态更新边界，见[审查报告](<./PROJECT-AUDIT-CN.md>)。

### 6.3 工具在哪里执行

[工具派发入口](<../server/src/cursor/tools/tool_call_dispatch/mod.rs#L34-L69>) 最适合读这一部分。

| 工具组 | 执行边界 | 阅读入口 |
| --- | --- | --- |
| Shell/Read/Delete/Grep/Glob/ReadLints/Task/MCP | 发协议给 Cursor，等 Exec/stream result | [exec 派发](<../server/src/cursor/tools/tool_call_dispatch/exec.rs>)、[工具编码](<../server/src/cursor/tools/codec/request.rs>) |
| Write/StrReplace/EditNotebook | 本地组装编辑协议，实际工作区操作交给 Cursor | [编辑派发](<../server/src/cursor/tools/tool_call_dispatch/edit.rs>) |
| AskQuestion/SwitchMode/CreatePlan | Cursor 交互返回结果 | [交互派发](<../server/src/cursor/tools/tool_call_dispatch/interaction.rs>) |
| WebSearch/WebFetch | 先收到 Cursor Approved，然后本地服务执行 HTTP | [交互续接](<../server/src/cursor/tools/tool_call_dispatch/interaction.rs#L25-L61>)、[search](<../server/src/search/mod.rs>) |
| TodoWrite/UpdateCurrentStep | 服务端构造结果/状态展示 | [本地派发](<../server/src/cursor/tools/tool_call_dispatch/local.rs>) |
| SembleSearch/SembleFindRelated | 服务进程直接访问指定 repo 并本地索引 | [搜索派发](<../server/src/cursor/tools/tool_call_dispatch/search.rs>)、[Semble 适配](<../server/src/search/search_provider.rs>) |
| 未知工具 | 兼容层返回失败，而不是自动增加执行能力 | [兼容层](<../server/src/cursor/tools/compat.rs>) |

接着读 [工具 runtime](<../server/src/cursor/tools/runtime.rs>) → [结果关联](<../server/src/cursor/tools/tool_call_result/mod.rs>) → [Run 的工具轮次](<../server/src/run/tool_round.rs>)。关注 call_id、exec_id、完成序号、重复结果、后台完成和取消，不要只看工具 schema。

Semble 的 repo 参数在当前实现没有与获准工作区 root 绑定，且远程 HTTP(S) repo 直接用系统 git 获取。不要把工具说明中的“本地/受限”当成已经做了权限隔离，相关边界见[审查报告 S12/S13](<./PROJECT-AUDIT-CN.md>)。

### 6.4 存储与恢复

先看 [SQLite 初始化](<../server/src/store/sqlite.rs>)，再按 0001→0009 顺序读[初始迁移](<../server/migrations/0001_initial.sql>)、[模型配置展平](<../server/migrations/0004_flatten_model_configuration.sql>)、[checkpoint 改名](<../server/migrations/0006_rename_revisions_to_checkpoints.sql>)、[调用历史外键调整](<../server/migrations/0007_drop_llm_calls_model_hash_fk.sql>)，不要只读初始表结构。

```text
messages（消息内容）
     ↑ checkpoint_messages（ordinal 排序关联）
conversation_checkpoints（父 checkpoint + digest）
     ↑ conversations（当前 head、active_run）
runs → tool_rounds → tool_round_calls
blobs/blob_edges → Cursor checkpoint、图片、trace artifacts
model_configs/service_settings → 模型凭据/设置
llm_calls → llm_call_requests/llm_call_response_chunks（详细模式下）
```

随后读 [Store checkpoints](<../server/src/store/checkpoints.rs>)、[CAS/blobs](<../server/src/store/cas.rs>)、[Cursor checkpoint builder](<../server/src/cursor/checkpoint/builder.rs>)、[消息 encode/decode](<../server/src/cursor/checkpoint/messages/mod.rs>)、[恢复](<../server/src/cursor/checkpoint/recovery.rs>)。

关闭详细调用日志不代表会话没有落盘：运行恢复仍需要 messages/checkpoints/tool results。数据库和插件资源都应视作敏感本地数据。

### 6.5 插件与可扩展能力

```text
plugins/build-in/<plugin>/plugin.json + main.ts
  → PluginRegistry 发现/装配
  → Deno worker（本地 SDK 与 stdin/stdout JSON 协议）
  → ctx.network.fetch/stream → Rust HostContext
  → HTTPS + manifest host allowlist → 目标服务
  → 统一 ModelEvent → 正常 Run 循环

OAuth/导入 → private resource → 本地 JSON 存储
  → 选择账号/刷新 token/同步模型 → provider invoke
```

读 [PluginRegistry](<../server/src/plugin/registry.rs>) 时不要一口气读完 1374 行：按“发现→资源 OAuth/导入→模型计划→stream→HostContext”的函数分组读。网络安全边界在 [worker](<../server/src/plugin/worker.rs#L467-L527>)；凭据在 [插件数据存储](<../server/src/plugin/data.rs>) 与 [资源 state](<../server/src/plugin/state.rs>)。

新增一种不兼容 OpenAI/Anthropic 的 API 需要新 provider 或插件；填写不同 key 不能自动适配任意协议。修改提示词则从 prompt/compile 开始，不要在每个 provider 里独立改历史。

## 7. 用测试代替猜状态

这些测试存在，但本次机器没有 Rust 工具链，**未执行以下 Rust 测试**。

| 要理解的行为 | 优先阅读的测试 |
| --- | --- |
| 请求解码、压缩、官方转发与 wire contract | [connect_wire](<../server/tests/connect_wire.rs>) |
| Transport 断开、替代 transport、清理 | [cursor_transport_lifecycle](<../server/tests/cursor_transport_lifecycle.rs>) |
| 同连接后续消息和后台通知 | [conversation_delivery](<../server/tests/conversation_delivery.rs>) |
| 中断/追加、旧工具 abort 与新消息 | [interrupt](<../server/tests/interrupt.rs>) |
| 工具 ID、顺序、拒绝、失败和后台 shell | [tool_round](<../server/tests/tool_round.rs>) |
| checkpoint、工具重放、图片恢复 | [checkpoint_recovery](<../server/tests/checkpoint_recovery.rs>) |
| 历史压缩和 usage/context 边界 | [compaction](<../server/tests/compaction.rs>) |
| 历史投影与前缀局部约束 | [prefix_stability](<../server/tests/prefix_stability.rs>) |
| 失败是否传播、连接怎样结束 | [error_lifecycle](<../server/tests/error_lifecycle.rs>) |
| 对外兼容 API 的 key 校验与协议转换 | [external_api](<../server/tests/external_api.rs>) |
| 本地规则注入与知识同步 | [local_rules_context](<../server/tests/local_rules_context.rs>)、[knowledge_rules](<../server/tests/knowledge_rules.rs>) |

推荐方法：选一个测试，先读 arrange 中的 protobuf 输入，再读 FakeProvider 输出，最后看期望的 AgentServerMessage/checkpoint。这样可以用很小的例子理解真实循环。

## 8. 最后读的旁路与安全边界

| 主题 | 入口 | 必须问的问题 |
| --- | --- | --- |
| 官方混用/默认模型 | [model_catalog](<../server/src/cursor/services/model_catalog.rs>)、[account](<../server/src/cursor/services/account.rs>) | 本地 RPC 是否仍先请求官方？ |
| 默认 TAB | [TAB 服务](<../server/src/cursor/services/tab.rs>)、[设置默认值](<../server/src/store/settings.rs>) | 补全、文件同步和账号 token 发给谁？ |
| 广告 | [广告服务](<../server/src/control/ads.rs>)、[ControlService](<../server/src/control/service.rs#L285-L345>)、[AppLayout 自动触发](<../apps/desktop/src/shell/AppLayout.tsx#L98-L185>)、[AdMenu](<../apps/desktop/src/shell/ads/AdMenu.tsx>) | 拉取、图片、关闭反馈分别如何外联？ |
| 更新 | [App 自动检查](<../apps/desktop/src/App.tsx#L53-L84>)、[updateStore](<../apps/desktop/src/shared/store/updateStore.ts>)、[Tauri update](<../apps/desktop/src-tauri/src/update/mod.rs>) | 自动检查与安装是否分开？谁持签名 key？ |
| WebSearch/WebFetch | [多源聚合](<../server/src/search/federation.rs>)、[目标检查](<../server/src/search/fetch.rs>) | query 发送给多少站点？是否防访问私网？ |
| 代码索引 | [embedding assets](<../crates/semble-core/src/embedding/assets.rs>)、[本地 source](<../crates/semble-core/src/source/local.rs>) | 首次要下载什么？读取 repo 的范围是什么？ |
| 本地管理权限 | [control router](<../server/src/control/mod.rs>)、[capabilities](<../apps/desktop/src-tauri/capabilities/default.json>) | localhost 是否被误当作鉴权？ |

[协议提取脚本](<../support/cursor-protocol-extractor/scripts/extract.sh>) 会更新协议文件，不是安全的“只读学习命令”；[抓包工具](<../support/cursor-capture/main.go>) 会启动服务、外联官方并保存流量，而且缺少已生成 Go protobuf 包。第一遍学习不需要执行它们。

## 9. 安全验证顺序与学习完成标准

有完整工具链后，在没有真实凭据、没有启动接管、没有开放公网端口的条件下，先做静态/离线测试，再考虑实机联调：

```bash
# 从仓库根目录执行；依赖准备会访问包仓库，不能称为离线步骤。
npm --prefix apps/desktop ci
cargo fmt --all -- --check
cargo test --package cursor-server --lib
cargo test --package cursor-server --test connect_wire
cargo test --package cursor-server --test tool_round
cargo test --package cursor-server --test checkpoint_recovery
cargo test --package cursor-server --test interrupt
cargo test --package cursor-server --test compaction
npm --prefix apps/desktop run check

# 完整项目检查（需 Tauri 各平台构建依赖）
make check
```

不要先执行 `make dev-desktop` 试一试：启动/读取接管状态可能应用已保存的 takeover 设置；启用接管还会结束 Cursor、修改 Cursor 设置，并涉及信任本地 CA。

读懂的验收标准：

1. 不看 README，能画出 BidiAppend → RunEngine → provider → 工具 → RunSSE 的闭环。
2. 能解释 request_id、conversation_id、run_id、checkpoint_id 为什么不同。
3. 能定位“配置成功但请求失败”的 URL、协议、模型标识、key、SSE/工具调用五个检查点。
4. 能分清 Cursor 执行的工作区工具、Rust 执行的搜索工具和 Deno 执行的 provider 插件。
5. 能说明默认配置会访问哪些第三方，哪些数据保存在本机。
6. 能用一个现有测试解释取消、续聊或恢复的状态变化；不会把所有声明的协议字段当作已实现功能。

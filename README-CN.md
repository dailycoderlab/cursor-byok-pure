# Cursor BYOK Pure

运行在本机的 Cursor 模型网关：移除运行时广告，默认使用内置 BYOK TAB，不使用作者公共 TAB 服务。

[English README](<./README.md>) · [源码仓库](<https://github.com/kaisun1120/cursor-byok-pure>) · [纯净模式说明](<./docs/PURE-MODE-CN.md>) · [源码阅读指南](<./docs/CODE-READING-CN.md>) · [MIT 许可证](<./LICENSE>)

> 本 fork 暂未配置更新源，点击检查更新仅提示“暂无更新源”。不要把上游发布包当作此纯净版。上游通用[安装教程](<https://docs.leokun.cn>)仍可参考，但广告、TAB 和更新行为以本仓库说明为准。

> “本地网关”不等于离线或绝对安全。所选模型 API 仍会外联，除非你配置的是已经运行的本地推理服务；本项目不会为 TAB 捆绑、下载或启动 Ollama/模型权重。其他 Cursor RPC、搜索、OAuth 和 Semble 下载仍有各自的网络边界。

以下截图已检查，没有可见广告；它们是**上游界面参考**，不是本 fork 实机验收证据，布局和文案可能已变化。

![上游控制面板参考](<./images/en-home-1.png>)

## 项目简介

cursor-byok 是一个开源的本地模型网关。它在你的设备上运行服务，接收 Cursor 发出的 Agent 请求，将请求转发到你配置的模型服务，并尽可能保留 Cursor Agent 的工具调用、Skills、MCP 和多轮对话能力。

你可以连接兼容 OpenAI 或 Anthropic 协议的服务，自定义服务地址、模型 ID、API Key 和请求参数，也可以使用 Cursor 平台默认选项之外的模型通道。

> [!IMPORTANT]
> cursor-byok 免费且开源，但你连接的模型服务商可能会按用量收费。本项目是独立项目，与 Cursor 或其开发者没有关联，也未获得其认可。

## 主要功能

- **自定义模型通道**：配置自己的 API 地址、凭据和模型 ID。
- **多种 API 协议**：支持 OpenAI Responses API、OpenAI Chat Completions API 和 Anthropic Messages API 兼容服务。
- **模型管理**：添加、复制、编辑、排序模型配置，并批量测试连接。
- **连接性能测试**：查看首字延迟、生成速度、总耗时和原始服务商响应。
- **Agent 工作流**：继续使用工具调用、Skills、MCP 和多轮对话。
- **会话指标**：查看 Token 用量、缓存命中率、对话轮次和估算价值。
- **内置 BYOK TAB**：默认本地模式，明确选择已配置模型后提供当前位置基本补全；也可显式选择官方直连或自托管服务。
- **无广告、无更新源**：广告链路已移除；保留 updater 机制，但自动检查不发更新请求，手动检查提示“暂无更新源”。
- **跨平台运行**：支持 macOS、Windows 和 Linux。

## 快速开始

本 fork 尚未发布安装包，请从本仓库经自己的审查/构建流程获取程序；上游发布包不包含本纯净版改动。

1. 打开 **Cursor 配置**，先阅读现有本地接管/CA 提示；接受其对系统信任和 Cursor 设置的影响后，由你自行启用接管并安装/信任 CA。
2. 在模型设置中填写协议、服务地址、API Key 和模型名称，保存并运行 **测试**。连接测试不等于 Agent 工具兼容或 TAB 显示验收。
3. 打开 **系统设置 → TAB 设置 → 编辑**，保持 **内置 BYOK TAB**，明确选择 **TAB 补全模型**并保存。未选择模型时停用，不自动使用 Agent 模型。
4. 保持桌面应用运行。TAB 复用同一个 Rust App，自然随应用启动/关闭，不需要另起 Node/Go 补全进程。
5. **首次接管、升级 Cursor 或首次配置模型后，完全退出并重新启动 Cursor，再新开 Agent 对话**，手动选择配置模型而非 Auto；TAB 另用非敏感临时文件验收。

以上是用户操作步骤，不表示本次文档任务已经启动应用、信任 CA 或修改实际 Cursor 设置。详细边界见[纯净模式说明](<./docs/PURE-MODE-CN.md>)；上游通用安装步骤可参考[中文使用指南](<https://docs.leokun.cn/zh/docs>)，不要照搬旧公益 TAB/下载行为。

> [!TIP]
> 首次升级 Cursor 或首次完成配置后，必须完全退出并重新启动 Cursor，再新开一个对话。配置前已经打开的对话不会加载新连接；使用自定义模型时，请在模型列表中手动选择该模型，不要选择 **Auto**。

## 模型配置

每个模型配置都是独立的上游通道，可以单独设置服务商、协议、凭据和生成参数。

![上游模型设置参考，可能与当前版本不同](<./images/en-model-1.png>)

### 类型与协议选择

| 模型系列 | 模型类型 | 请求协议 |
| --- | --- | --- |
| Claude 系列 | **Anthropic** | Messages API |
| GPT / OpenAI 系列 | **OpenAI** | **Responses API** |
| 其他模型 | **OpenAI** | **Chat Completions API** |

GPT 系列建议使用 **Responses API**。如果使用 Chat Completions，可能无法保留提示词缓存，导致速度变慢和费用增加。

### 常用字段

- **模型类型**：选择 OpenAI 或 Anthropic，决定上游接口格式。
- **请求协议**：OpenAI 类型需要继续选择 Responses API 或 Chat Completions API。
- **服务器地址**：可以填写服务商基础地址，让应用按协议追加标准端点，也可以填写完整请求 URL 并原样使用。
- **API Key**：填写上游服务要求的访问密钥。密钥保存在本机，用于发送模型请求。
- **模型名称**：填写服务商接口接受的模型标识，也可以使用 **获取模型** 读取服务商返回的模型列表。
- **显示名称**：Cursor 模型列表中显示的名称，不会改变发送给上游的模型标识。
- **备注**：显示在 Cursor 的模型说明中。

还可以根据模型能力设置上下文窗口 Token、最大输出 Token、推理或思考强度、自定义 Headers，以及 OpenAI 或 Anthropic 的额外参数。自定义 Headers 和额外参数必须是 JSON 对象，只应填写服务商明确支持的字段。

保存配置后运行 **测试**，确认地址、协议、API Key、模型标识和流式响应都正常，再在 Cursor 中使用该模型。

## TAB 补全服务

TAB 不走 Agent 会话/工具循环，而是在现有 Rust 服务中独立处理；本地模式复用你明确选中的模型通道。

| 模式 | 目标 | 条件与边界 |
| --- | --- | --- |
| **内置 BYOK TAB（默认 Local）** | 选定的配置模型/provider | 后端不依赖官方账号；`model_id` 为空就停用，不调用 provider、不外部回退、不自动选 Agent 模型 |
| **Cursor 官方直连（显式 Direct）** | 官方 TAB | 需要真实登录和可用额度；不承诺匿名可用，mock Ultra 不授予上游权益 |
| **自托管服务（显式 Custom）** | 你输入的服务基础 URL | 自行审查信任/鉴权；不是自动本地推理 |

设置为 `{mode, address, model_id}`，新存储键是 `cursor_tab_service`。**旧 `cursor_tab` 不读取、不迁移、不加兼容别名，旧的显式 Custom 地址也不会沿用**：请重新选择模型或输入自托管 URL。其他模型、Commit、账号数据不重置。

本地 TAB 只使用请求携带的当前文件快照，生成光标处短插入，再返回“原当前行前缀 + 补全文本 + 原后缀”的当前行替换；处理 UTF-16 光标列和 CRLF。不扫描磁盘仓库、不使用文件同步缓存、不支持多位置/下一光标预测、import/fused/chunks 或分支名生成。缺少文件内容的 file-sync 请求必须重传快照。

默认 256 输出 tokens、最多 4096 生成字节/16 行、256 KiB 输入、8 秒或更短客户端期限、单并发无队列；断开响应取消 provider 并释放许可。用户额外参数可能覆盖上游 token 设置，**256 不是费用上限保证**。不执行工具、不进入 Agent checkpoint 循环、不把 reasoning 当代码输出。

旧 archive 的 `cursor-tab-server` 是需要官方 token 的 Go 代理，不是本地推理/免账号方案。完整结构、操作步骤和未验证客户端行为见[纯净模式说明](<./docs/PURE-MODE-CN.md>)。

## 与官方账号并存

新版设计支持 cursor-byok 与 Cursor 官方服务并存：

- 直接在 Cursor 中登录自己的账号。如果之前使用旧版生成的 fake 账户，请先退出该账户，再登录自己的账号。
- 账号拥有官方额度时，官方模型和本地模型可以随时切换混用。
- **Auto 只使用官方模型**，不会自动使用你配置的本地模型。账号没有官方额度时，请手动选择自己配置的模型。
- 未接管的插件/代码库索引等功能仍按 Cursor 自身的网络、账号和额度规则运行，不保证完全离线可用。
- Local TAB 不把官方凭据头送给模型 provider；显式 Custom 仍透传原 RPC，可能包含 Authorization/Cookie，请只使用自己审查信任的目标。

## 数据流转

```text
Cursor → 用户自行启用的既有 CA/本地代理 → 同一个 Rust App
    ├─ Agent → 会话/checkpoint/工具循环 → 你配置的模型 API
    ├─ Local TAB → 请求自带快照 → 一次受限 provider 调用 → 当前行替换
    ├─ Commit → 选定 Commit 模型；未选择时官方直连（不跟随 TAB）
    └─ 未接管 RPC / 显式 Direct TAB → 既有官方转发

桌面设置 → 本地 SQLite → 模型/TAB 配置
桌面 updater → 无更新源 → 不发更新请求
```

API Key、模型配置和应用设置保存在本机；请求仍发给你选的服务商。TAB metadata 保留 `api2.cursor.sh` 的正常 origin，供已启用的现有代理路由回本机，不是 Local 生成失败后的官方回退；未启用代理时不能保证请求到达本地。

广告拉取、图片、关闭反馈、安装 ID 创建、前端缓存/localStorage 和相关翻译已移除，`react-css-marquee` 依赖也已移除。历史广告缓存/数据库标识不再读取复用，但没有删除真实用户目录、数据库或缓存。

保留 updater 检查/验签/安装机制，Tauri `endpoints=[]`、Windows portable 源为 `None`；无源时启动/聚焦/可见性检查不调用更新插件/IPC，手动检查与缓存待安装项都提示“暂无更新源”。未来发布需配置自己的源及正确签名，而不是只换一个 URL。

**管理 API 无鉴权、OAuth/详细日志/插件删除/Semble 等风险没有随广告一起修复。** 保持 loopback，先用低权限 Key 和非敏感文件验证，见[历史审查与当前改动](<./docs/PROJECT-AUDIT-CN.md>)。

## 项目结构

```text
cursor-byok-pure/
├── apps/
│   └── desktop/         # 桌面应用
│       ├── src/         # React 管理界面
│       │   ├── features/ # 首页、模型、调用记录与设置
│       │   ├── shell/    # 窗口框架与页面布局；广告模块已删除
│       │   ├── shared/   # UI、虚拟列表、状态、API 与平台能力
│       │   ├── i18n/     # 本地化运行时与语言目录
│       │   └── styles/   # 全局主题与排版令牌
│       └── src-tauri/    # Tauri 桌面生命周期
├── server/
│   ├── src/
│   │   ├── cursor/    # Cursor 协议适配；services/tab/ 为内置补全
│   │   ├── run/       # 通用 Agent Runtime 与内部 Port
│   │   ├── provider/  # 模型供应商适配
│   │   ├── model/     # 聚合领域模型
│   │   ├── store/     # SQLite Repository
│   │   ├── control/   # 管理面 API
│   │   ├── local_app/ # Cursor 本地 CA、代理、设置与账号接管
│   │   └── search/    # Web 与 Semble 搜索接入
│   ├── prompt/
│   └── migrations/
├── crates/
│   └── semble-core/   # 本地代码索引与搜索核心库
├── protocols/
│   └── cursor/        # Cursor 协议定义的唯一来源
├── support/                     # 仓库辅助设施
│   ├── cursor-protocol-extractor/ # Cursor 协议提取与 Go 代码生成工具
│   ├── cursor-capture/            # Cursor 协议抓取和调试工具
│   └── benchmarks/                # 代码搜索和索引基准测试
├── images/            # README 展示图片
├── Cargo.toml         # Rust 工作区配置
└── Makefile           # 常用开发、检查和构建命令
```

## 本地开发

### 环境要求

- Rust 工具链和 Cargo
- Node.js 与 npm
- Tauri 2 的系统构建依赖
- Docker（仅在构建 Docker 镜像时需要）

### 安装依赖

```bash
npm --prefix apps/desktop install
```

### 启动开发环境

启动桌面前端：

```bash
make dev-web
```

启动桌面应用：

```bash
make dev-desktop
```

### 检查与构建

运行完整检查：

```bash
make check
```

分别构建各部分：

```bash
make build-web       # 构建桌面前端
make build-server    # 构建 Rust 本地服务
make build-desktop   # 构建 Tauri 桌面安装包
make build-docker    # 构建 Docker 镜像
```

## 验证与反馈

已记录无安装脚本的前端依赖安装、i18n 完整性、TypeScript/Vite 检查与 Rust 定向 45 项测试通过；精确范围见[验证表](<./docs/PURE-MODE-CN.md#6-验证结果与未验证项>)。协议 fixtures 不是 Cursor 已显示/接受 ghost text 的证明；真实 Cursor、真实模型及官方登录/匿名行为仍需单独验收。

- [本 fork 源码与问题反馈](<https://github.com/kaisun1120/cursor-byok-pure>)
- [上游源码归属](<https://github.com/leookun/cursor-byok>)
- [上游通用中文使用指南](<https://docs.leokun.cn/zh/docs>)（与本 fork 行为区分）

提交问题时，请附上操作系统、cursor-byok 版本、模型类型、请求协议、已脱敏的服务地址、错误信息和复现步骤。请勿公开 API Key 或其他凭据。

## 参与贡献

欢迎提交 Issue 和 Pull Request。提交代码前请先阅读项目中的开发说明，并运行 `make check` 确认格式、测试和前端构建检查通过。

## 许可证

本项目采用 [MIT License](<./LICENSE>) 开源，保留原项目版权与来源归属；纯净化不改变许可证。

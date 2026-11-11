# 纯净模式：内置 BYOK TAB、无广告、无更新源

**当前实现已移除广告和作者公共 TAB 默认路径；TAB 默认官方直连；切换到内置 Local TAB 后只调用你明确选中的配置模型。updater 功能保留，但所有更新源为空，手动检查显示“暂无更新源”。** 这些改动不等于完全离线、官方后端全功能替代或安全问题全部修复。

本说明描述 2026-11-05 的实现。旧快照问题与当前修复的区别见[历史审查](<./PROJECT-AUDIT-CN.md>)，Agent 主线见[阅读指南](<./CODE-READING-CN.md>)。

## 1. 当前目录与数据流

以下为实测物理行数，含空行、注释和文件内测试；TAB 四个生产文件共 **852 行**，独立测试不计入该总数。旧 51 行 `services/tab.rs` 已删除，不是兼容入口。

```text
apps/desktop/
├── src-tauri/
│   ├── src/desktop.rs                     # 现有 Rust App/窗口/退出生命周期，320 lines
│   ├── src/update/                        # 保留更新验签/安装/Windows 替换机制
│   └── tauri.conf.json                    # updater.endpoints=[]，保留 pubkey
└── src/
    ├── features/settings/TabSettingsCard.tsx  # 模式与明确模型选择
    ├── shared/native/appLifecycle.ts      # 读取 Tauri 源配置，无源时短路更新调用
    └── shell/                             # 窗口/导航；原 ads/ 子树已删除
server/src/
├── app.rs                                 # 组装同一个 App/Store/provider
├── local_app/                             # 既有 Cursor CA/代理/设置/账号接管
├── store/settings.rs                      # cursor_tab_service；不读旧键
├── cursor/services/
│   ├── tab/                               # 内置基本补全，不走 Agent 循环
│   │   ├── mod.rs                         # RPC/模式/限流/元数据，285 lines
│   │   ├── proto.rs                       # 最小 TAB protobuf 契约，150 lines
│   │   ├── context.rs                     # 快照裁剪、UTF-16/CRLF、当前行替换，215 lines
│   │   └── completion.rs                  # 单次 provider、取消/期限/Connect 输出，202 lines
│   └── commit_message.rs                  # Commit 模型独立配置；未选则官方直连
└── control/                               # 管理 API；原 ads.rs 已删除
server/tests/
├── tab_service.rs                         # fake provider / TAB 协议与取消边界
└── pure_mode.rs                           # 旧配置隔离、广告/公共服务等回归
```

```text
Cursor 当前文件快照 + UTF-16 光标
  → 用户启用的既有 CA/本地代理（Local TAB 路径）
  → tab/mod.rs：读取 mode/model_id，拒绝缺配置/超大输入
  → context.rs：只裁剪该快照，不读磁盘/仓库/同步缓存
  → 单并发许可（占用即拒绝，不排队）
  → completion.rs：一次 provider 调用 → 你选的模型 API
  → 验证短代码 → 原当前行前缀 + 插入 + 原后缀
  → Connect begin_edit / text / done_edit / 正常结束 → Cursor
       客户端是否显示并接受 ghost text 仍待实机验证
```

TAB 的参数、模式保存在已有 SQLite 设置表；每次生成只拥有当次快照和取消令牌，不创建 Agent 会话/checkpoint，不进入“模型→工具→继续”循环。模型 API 在远端时，裁剪后的代码仍会发送给它。

**生命周期：** 桌面启动 → 既有 Rust App 组装 → 接管开启后处理 TAB；每次响应持有 provider 流、取消令牌和许可；响应 Body 被丢弃 → 取消 provider、释放许可。关闭主窗口时服务仍在托盘后台运行；从菜单显式退出程序才取消并停止同一个内置服务。没有新 Node/Go 守护进程，也没有 bundled inference runtime。退出不意味着自动卸载系统 CA 或完全恢复 Cursor 原设置。

## 2. 模式和配置契约

| 设置 | 默认/目标 | 行为与要求 |
| --- | --- | --- |
| `Local` | 显式选择；随桌面应用启动的内置 TAB 服务，选定模型/provider | 后端没有官方账号依赖；模型可为已配置普通通道或插件模型（插件自身可能需要它的账号） |
| Local `model_id=""` | 未选模型 | 明确停用：不调用 provider、不回退官方/作者服务、不自动选择 Agent 模型 |
| `Direct` | 默认；官方 TAB | 走官方服务，需真实登录与可用额度；不承诺匿名 TAB |

设置对象为 `TabSettings { mode, model_id }`。`model_id` 保存配置模型的稳定标识（普通模型 hash 或插件 ID），不是自动取当前 Agent 模型。

**无兼容/迁移：** 存储键由 `cursor_tab` 改为 `cursor_tab_service`；旧键完全不读取，不加 alias，自托管（Custom）模式已删除。使用内置 TAB 须重新选择模型。其他模型、Commit、账号数据不重置；旧广告缓存/数据库标识不读不复用，也没有删除任何真实用户目录、数据库或缓存。

Commit 与 TAB 已解耦：Commit 选模型则本地生成；未选模型仍是官方直连，**不是停用，也不跟随 TAB 模式**。敏感 diff 若不希望发官方，请单独选择 Commit 模型。

Local 生成不会透传 Cursor 官方凭据给模型 provider，只使用选定模型通道自己的凭据。Direct 模式仍复用原 RPC 透传，不会剥离原请求的 Authorization/Cookie 等账号头；若客户端携带这些头，它们会随 payload 进入自托管目标。只填自己已审查信任的服务，不把“默认公共服务移除”误解为所有跨目标凭据风险已解决。

官方产品与此 fork 要区分：官方[API Keys 说明](<https://cursor.com/help/models-and-usage/api-keys>)将自定义 key 用于 chat，Tab 使用内建模型；[Tab 故障说明](<https://cursor.com/help/troubleshooting/tab-issues>)涉及 Free/Hobby 限额与网络；[Quickstart](<https://cursor.com/docs/get-started/quickstart>)有登录步骤。不能据此声称官方匿名 Tab 有保证，mock Ultra 也不是上游授权。旧 archive 的 `cursor-tab-server` 是要求官方 token 的 Go 代理，不是本地推理/免账号方案。

## 3. 设置步骤（由用户自行操作）

1. **获取并运行本 fork 的新构建。** 本次未发布 release；不能下载上游安装包当纯净版。若旧桌面程序正在运行，先从托盘菜单彻底退出再启动新构建；改源码不会更新旧进程，同一 single-instance 标识还可能把新启动转交旧程序。通用[上游教程](<https://docs.leokun.cn>)可以保留作参考，应用内教程是点击后确认打开，不是自动外联教程站。
2. **审查并启用既有本地接管。** 打开 Cursor 配置，了解代理、CA 和 Cursor 设置改动；由你自行安装/信任 CA 并启用接管。没有这些条件，TAB 的正常官方 origin 不一定路由回本地。
3. **添加并测试模型通道。** 填协议、地址、Key、模型名称，保存并运行连接测试。推荐低延迟、短输出模型；连接测试不是工具能力或客户端补全验收。
4. **明确选择 TAB 模型。** 系统设置 → TAB 设置 → 编辑 → 内置 BYOK TAB → TAB 补全模型 → 保存。不选模型就是停用，无需再填第二套 Key。若从旧版本更新，须重新选择模型。
5. **保持桌面运行并重启 Cursor。** 首次接管/升级后完全退出并重启 Cursor；Agent 新开对话并手动选配置模型（不是 Auto）；TAB 另在非敏感临时文件尝试行尾、行中、中文/emoji、CRLF。
6. **检查更新行为。** 系统设置 → 应用设置 → 检查更新，应提示“暂无更新源”，不是“当前已是最新版本”。这不是需要你补填作者更新源的故障。

本次实现/文档工作没有代用户启动应用、安装或信任 CA、修改实际 Cursor 设置、调用真实模型、下载权重或启动 Ollama。若需要真正本地推理，先由你准备可用的本地兼容 API，再将其作为模型通道选择；本项目不负责准备推理运行时。

## 4. 已实现能力与故意不支持的边界

| 类别 | 当前行为 |
| --- | --- |
| `StreamCpp` | 请求携带当前文件内容/光标；支持完整或带起始行的快照、UTF-16 列、CRLF；将当前位置短插入编码为原当前行替换 |
| 输出 | 只返回代码；拒绝工具调用，不把 reasoning 事件输出成补全文本；不改用户模型的持久推理配置 |
| 预算 | 默认上游输出 256 tokens；生成文本最多 4096 字节/16 行，输入 body 256 KiB；按模型上下文窗口裁剪 prefix/suffix |
| 期限/并发 | 全请求 8 秒或更短 `connect-timeout-ms`；一次并发，无队列；响应被丢弃取消 provider、释放许可 |
| metadata | 本地返回 CppConfig/AvailableModels，仅选定且存在的配置模型标记启用；不为此请求调用官方 |
| edit classification | 配置模型有效时 `should_noop=false`，未选/失效时为 true；只允许现有基本建议继续，不提供 learned edit ranking |
| 文件同步 | 明确 disabled；Upload/Sync 返回 `NON_EXISTANT`，不假装同步成功；`rely_on_filesync` 且无 contents 必须重传快照 |
| 历史/指标 | 在本地处理/返回禁用或空结果，不转发作者服务；不积累 TAB 文件同步/编辑历史缓存 |
| 下一光标/多文件预测 | 不支持，`StreamNextCursorPrediction` 正常结束空流 |
| import/fused/chunks | 禁用，不模拟这些专有能力 |
| branch generation | 不支持，明确错误 |
| 未知补全 RPC | CppService/FileSyncService 整个 namespace，以及 AiService 方法名含 Cpp/Tab，在 Local 路由与 API fallback 中 fail closed |

**限制不是计费保证：** 用户显式额外参数仍可能覆盖 provider 的 upstream max-output 参数；默认 256 tokens 不是硬性 API 费用上限。8 秒期限和本地字节限额也不能证明服务商一定停止计费或取消已执行工作。

**不是全局网络防火墙：** fail closed 只覆盖当前识别的补全 namespace/命名规则。未来使用其他名字的 RPC、其他 Cursor 服务和 Cursor 自身流量不被全局阻断。Direct 是对官方的外联；不要把 Local 的默认边界套给它们。

Cpp metadata 中 `cpp_url`/`geo_cpp_backend_url` 保留正常 `https://api2.cursor.sh` origin，以复用已启用的既有 CA/本地 relay；**不是 Local 生成失败后向官方发代码**。代理未启用或客户端不走该路径则需单独诊断。

## 5. 广告与 updater 的处理

- 后端广告管理模块、前端广告子树全部删除；广告拉取、图片下载/加载、关闭原因反馈、installation ID 创建、UI、缓存/localStorage、相关翻译均移除。
- `react-css-marquee` 从 package/lock 移除；README 移除 BaoCode/Trendshift/远程推广徽章与误导的上游下载链接。保留许可证、版权、上游来源和经用户允许的通用教程；原图片/许可证文件未删除。
- **保留 updater，不保留任何更新源：** Tauri 配置 `endpoints=[]`，原签名公钥/验签与安装机制未改；Windows portable endpoint 是 `Option::None`。
- 前端读取 Tauri JSON 作为源配置事实：自动启动、focus、visibility 检查遇无源直接返回，不调用 updater 插件或更新 IPC；手动按钮本地化提示“暂无更新源”，缓存待安装项也被阻止。
- 后续若发布自己的版本，必须审查自己的 update source **及签名配置/发布链**，包括 Windows portable 源。只替换下载 URL 不足以建立自己的信任链；当前没有发布新 release。

## 6. 验证结果与未验证项

以下为主代理最终验证结果；构建和测试使用忽略目录内的独立 Rust/Cargo 环境与测试 HOME，没有启动桌面程序或操作真实 Cursor。

| 检查 | 已记录结果 | 不代表什么 |
| --- | --- | --- |
| 前端依赖安装 | `npm ci --ignore-scripts`：175 packages | 不是全部依赖内容/供应链审计；未运行依赖安装脚本 |
| i18n | 扫描通过；zh/en/pt 三语言 455 keys 完整 | 不是所有运行界面的视觉/可访问性验收 |
| 前端检查 | `npm run check` 通过：frontend + node TypeScript、Vite | 不是 Tauri 原生打包/桌面运行验收 |
| Rust 格式/定向回归 | `cargo fmt --all -- --check` 通过；最终 pure_mode 8、tab_service 21、commit_message 5 均通过 | classifier 回归已计入；不是实机客户端验收 |
| Rust 全 workspace | `cargo test --locked --workspace -- --test-threads=1`：**332 passed / 0 failed**，exit 0；测试 HOME 隔离，显式设置 loopback `NO_PROXY` | 可选 embedding 模型夹具未配置，其已有测试提前返回；未验证真实推理/OAuth |
| macOS 原生构建 | `npm --prefix apps/desktop run tauri -- build --debug --no-bundle` 通过，得到 arm64 Mach-O；未运行 | 不是签名安装包、Windows/Linux 原生构建或真实桌面联调 |
| 真实 Cursor/模型 | **未做** | fixtures 不证明 range/ghost text 能显示、TAB 能接受或 full-buffer fallback 会实际触发 |
| 官方行为 | **未做匿名/真实登录/额度测试** | 不承诺原生 Direct 不登录可用 |

**扩大验证的环境边界：** 普通并发 workspace 两次分别在未改动的 Provider HTTP 传输断言上失败；其中一个单独复测通过。最终采用单线程和 loopback `NO_PROXY` 全套通过；未修改该 Provider 实现或掩盖失败。已有 Custom 透传的无 `Content-Length` synthetic HTTP/1 请求曾得到空正文；明确长度的 fixture 通过，通用 proxy 未修改，不据此保证所有无长度/流式请求兼容。

实机验收至少需明确 Cursor 版本，使用非敏感文件验证：当前位置插入和当前行替换范围、UTF-16/CRLF、缺快照重传、ghost text 显示/接受、取消后新请求、模型失效不外部回退。server fixtures 只证明本地协议/行为，不是客户端兼容证明。

## 7. 仍需单独处理的风险

已修复的历史项限于 S1 的作者公共 TAB/Commit 默认路由、S6 广告图片 SSRF 入口，以及广告/上游更新源；不把全部审计项标成完成。

- **S2 管理 API 没有真实鉴权**，仍可读写模型/设置；默认 loopback 降低暴露面，不是安全授权。不要把服务端口暴露到局域网/公网。
- OAuth 明文导出、refresh body/response 进入详细日志、插件删除 `.`/`..` 边界、明文凭据、插件信任与原生权限问题未随此改动解决。
- Semble 工作区范围/系统 git/缓存/下载边界、搜索 query 多源外联仍存在；Local TAB 不读仓库不等于 Agent 的其他工具也不读。
- 接管影响系统 CA 和 Cursor 设置；关闭接管开关会让 Cursor 恢复原厂状态（停止代理、恢复原设置、移除注入的本地账号、退出 Cursor），再次开启才重新接管；退出桌面应用只停止代理并恢复设置、保留开关状态；卸载证书/清理数据仍是独立用户操作。

保持低权限测试凭据、非敏感文件和 loopback；若要求“只访问我的模型 API”，还需要单独设计并验证网络出口白名单与功能损失。**Pure ≠ offline ≠ secure。**

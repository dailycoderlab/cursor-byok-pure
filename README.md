# Cursor BYOK Pure

A local model gateway for Cursor: runtime ads removed, built-in BYOK TAB available as an explicit option, and no author-hosted public TAB service.

[中文说明](<./README-CN.md>) · [Source](<https://github.com/dailycoderlab/cursor-byok-pure>) · [Pure-mode guide (Chinese)](<./docs/PURE-MODE-CN.md>) · [Code-reading guide (Chinese)](<./docs/CODE-READING-CN.md>) · [MIT License](<./LICENSE>)

> This fork has no configured update source. The updater is retained, but **Check for updates** reports **暂无更新源** in Chinese (localized in other languages). Do not install an upstream release expecting this fork's behavior. The upstream [installation guide](<https://docs.leokun.cn>) remains useful for general setup; TAB, ads and updates follow this repository's documentation.

## About

Cursor BYOK Pure runs the existing Rust service on your machine, adapts Cursor Agent requests to your configured model APIs, and supports Agent tool calling, Skills, MCP and multi-turn conversations. It is not a complete replacement for every Cursor backend service.

Connect OpenAI Chat Completions, OpenAI Responses or Anthropic Messages-compatible services with your own endpoint, model ID, API key and parameters.

**Local gateway does not mean offline or universally secure.** Model requests still leave your machine unless you select an already-running local inference API. Other Cursor RPCs, optional search, OAuth plugins and Semble asset downloads may also use the network. No inference runtime or model weights are bundled, downloaded or started for TAB.

This is an independent project, not affiliated with or endorsed by Cursor. The gateway is free and open source; your model provider may charge for usage.

## Features

- Configure, duplicate, edit, reorder and test model channels.
- Use the three compatible model API protocols with custom headers and parameters.
- Preserve the existing Agent conversation, tool and checkpoint loop.
- Inspect connection latency, provider responses and session metrics.
- Use **built-in BYOK TAB** for basic current-position completion with an explicitly selected configured model.
- Select **Cursor official direct** or **self-hosted service** only when explicitly desired.
- Run the desktop application on macOS, Windows or Linux; this change has not been accepted in a real Cursor client on all platforms.

## Quick start

This fork has not published a release. Obtain a build from this repository through your own reviewed build process; upstream downloads are not Pure builds. Development prerequisites and commands are in the [development instructions (Chinese)](<./README-CN.md#本地开发>).

1. Open **Cursor Configuration**. Review the existing local takeover and CA instructions, then enable/trust them yourself if you accept their effect on Cursor settings and system trust.
2. Add a model endpoint, protocol, API key and model ID; save and run the model connection test. That test is not proof of Agent tool compatibility or TAB rendering.
3. Open **System settings → TAB settings → Edit**. Keep **Built-in BYOK TAB**, explicitly select **TAB completion model**, and save. An empty selection disables local TAB; the Agent model is never selected automatically.
4. Keep the desktop application running. Its built-in TAB service shares the existing Rust App's startup/shutdown; no extra Node or Go service is needed.
5. Completely restart Cursor after first-time integration or a Cursor upgrade. Use a new Agent conversation and explicitly choose your configured model, not **Auto**. Check TAB separately in a non-sensitive scratch file.

These are user setup instructions, not a claim that this documentation task installed a CA, changed Cursor settings or launched an application.

## TAB modes and configuration reset

| Mode | Destination | Requirement / boundary |
| --- | --- | --- |
| Direct (default) | Cursor's official TAB service | Valid official login and available quota; anonymous TAB is not promised |
| Built-in BYOK TAB (`local`, explicit) | Selected configured model through the existing provider router, served by the desktop app's own Rust service | No official-account dependency in the backend; empty `model_id` means disabled, with no provider call or external fallback |

TAB settings are now `{mode, model_id}`, stored under `cursor_tab_service`. The old `cursor_tab` key is **not read, migrated or aliased**, and the self-hosted (Custom) mode has been removed. Reselect a model if you use the built-in TAB. Other model, Commit and account data is not reset.

Local TAB uses only the current buffer snapshot supplied by the request. It returns a replacement of the original current line that inserts short generated text at the UTF-16 cursor position while preserving its prefix/suffix and CRLF. It does not scan your repository or use a filesystem-sync cache. Next-cursor/multi-file prediction, import/fused prediction, chunks and branch generation are unsupported. If Cursor sends only a file-sync reference without contents, it must resend the buffer.

The default request is 256 output tokens, at most 4,096 generated bytes / 16 lines, with a 256 KiB input limit, an 8-second deadline (or a shorter client deadline), and one concurrent generation without a queue. Explicit provider extra parameters may override the upstream token setting: **256 tokens is not a guaranteed billing ceiling**. Dropping the response cancels the provider request and releases the permit. There are no tools or Agent conversation/checkpoint iterations; reasoning is not returned as code.

Local TAB uses only the selected provider's credentials, not Cursor account headers.

See the [pure-mode guide](<./docs/PURE-MODE-CN.md>) for the supported RPC boundaries and unverified client behavior. The old archived `cursor-tab-server` is a Go proxy requiring an official token, not bundled local inference or a no-account solution. Official Cursor BYOK Pure chat and official Tab are separate capabilities; mock Ultra responses do not authorize upstream access.

## Updates and removed advertising

- Runtime ad requests, images, dismissal feedback, installation-ID creation, UI, caches, localStorage usage and related translations were removed, together with `react-css-marquee`.
- Historical ad identifiers/cache entries are not read or reused. No real user data directory, database or cache was deleted.
- Updater checks, signing verification and installation mechanisms remain. Tauri endpoints are empty and the Windows portable source is `None`.
- Startup/focus/visibility checks return without updater-plugin or update IPC requests. Manual checks report **no update source**, not **already up to date**; cached pending installations are also blocked.
- A future release needs your own reviewed update sources **and signing configuration**. No release was published by this change.

## Data flow

```text
Cursor client → user-enabled existing CA/local relay → existing Rust App
    ├─ Agent → conversation/checkpoint/tool loop → configured model API
    ├─ Local TAB → supplied buffer → one bounded provider call → current-line edit
    ├─ Commit → configured Commit model, or official direct when unselected
    └─ Other RPCs / explicit Direct TAB → existing official forwarding

Desktop settings → local SQLite → selected models / TAB mode
Desktop updater → no configured source → no update request
```

The normal `api2.cursor.sh` origin remains in TAB metadata so the active existing relay can route requests locally; it is not a local TAB generation fallback to Cursor's servers. This only applies when that relay is enabled.

Management APIs still lack authentication, and the historical audit's OAuth/logging/plugin deletion/Semble risks were not fixed by removing ads. Keep the listener on loopback and use low-privilege keys/non-sensitive files until separately reviewed. See the [historical audit and current delta](<./docs/PROJECT-AUDIT-CN.md>).

## Illustrations

These inspected upstream screenshots have no visible ads, but are **upstream references**, not screenshots or acceptance evidence of this fork. Labels and layouts may differ.

![Upstream dashboard reference](<./images/en-home-1.png>)

![Upstream model-settings reference](<./images/en-model-1.png>)

## Validation boundary

The recorded checks include frontend dependency installation without lifecycle scripts, localization completeness, TypeScript/Vite checks, and a targeted 45-test Rust batch. See the [validation table](<./docs/PURE-MODE-CN.md#6-验证结果与未验证项>) for exact scope. No real Cursor E2E, real model/official-login test or client ghost-text acceptance has been established by these fixtures.

## Source, contribution and license

Fork source and issues: [dailycoderlab/cursor-byok-pure](<https://github.com/dailycoderlab/cursor-byok-pure>). Original project: [leookun/cursor-byok](<https://github.com/leookun/cursor-byok>). General upstream tutorial: [docs.leokun.cn](<https://docs.leokun.cn>).

Read [AGENTS.md](<./AGENTS.md>) and the [development instructions (Chinese)](<./README-CN.md#本地开发>) before changing code. Preserve the original attribution and copyright notices. This project remains licensed under the [MIT License](<./LICENSE>).

# Router Switch

用纯 Rust + [GPUI](https://www.gpui.rs/) + [gpui-component](https://github.com/longbridge/gpui-component) 复刻 [CC Switch](https://ccswitch.io) 的桌面配置中枢。不嵌入 WebView / React。

当前竖切只打通 **Codex**：供应商 CRUD、启用时原子写入 `~/.codex`，编辑当前供应商时从 live 回填。Claude Code / Grok Build 在侧栏占位。

## 运行

```bash
cargo run -p router-switch
```

需要 Rust 1.85+（gpui-component 0.5.1 使用 edition 2024）。本机已验证工具链：`rustc 1.93.1`。

```bash
cargo test -p domain -p adapters-codex -p store -p session
```

## 数据目录

| 路径 | 用途 |
|---|---|
| `~/.router-switch/app.db` | 应用 SSOT（供应商、当前启用、设置） |
| `~/.codex/auth.json` | Codex live 登录材料 |
| `~/.codex/config.toml` | Codex live 模型/端点 |

可用环境变量覆盖：

- `ROUTER_SWITCH_HOME`：应用数据目录
- `CODEX_HOME`：Codex live 目录

不会读写 `~/.cc-switch/`。

## Codex 行为

- **官方** `codex-official`：空 auth / 空 config。启用时只整理 `config.toml`，**不覆盖**已有 ChatGPT OAuth。未登录时在终端执行 `codex login`。
- **Responses 第三方**：API Key + 端点。写入 `{ "OPENAI_API_KEY": "..." }` 以及 `wire_api = "responses"` 的 `[model_providers.custom]`。
- 双文件写入是原子的；`config.toml` 失败会回滚 `auth.json`。
- 启用后必须**重启 Codex / 终端**。
- 编辑当前供应商时从 live 回填 key / base_url / model。

## 仓库

```
crates/domain           纯函数：表单校验、TOML/auth 模板
crates/adapters-codex   读写下 ~/.codex
crates/store            SQLite SSOT
crates/session          供应商 CRUD + 启用写 live
crates/ui               GPUI 窗口
crates/app              入口
```

Claude / Grok 的 live adapter、托盘与系统通知按计划后置。
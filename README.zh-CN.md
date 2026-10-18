<div align="center">

# token-speed

**一眼看清你的编程 Agent。**

面向 Codex、Claude Code、DSH（DeepSeek Harness）以及任何接入 Magpie 网关的 Agent 的终端观测台。<br>
四个视图，读取本地日志，一个原生 Rust 可执行文件。<br>
<sub>Fork 自 [william1010121/token-speed](https://github.com/william1010121/token-speed)，新增 DSH 与 Magpie 网关数据源。</sub>

[![CI](https://github.com/libukai/token-speed/actions/workflows/ci.yml/badge.svg)](https://github.com/libukai/token-speed/actions/workflows/ci.yml)
[![Rust](https://img.shields.io/badge/Rust-1.92%2B-dea584?logo=rust)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-74b9ff)](LICENSE)

[快速开始](#快速开始) · [视图](#四种视图) · [命令行](#脚本友好) · [度量说明](docs/metrics.md) · [English](README.md)

<img src="docs/preview.png" alt="token-speed 总览：各来源吞吐卡片、活动图、最近请求和模型对比" width="1100">

<sub>所有截图均为合成的演示数据。</sub>

</div>

## 为什么用 token-speed？

用量日志你本来就有。不用离开终端，就能看到 Agent 什么时候在干活、在你的实际工作中哪个模型更快、哪些请求值得细看。

- **四个联动视图：** 总览、时间线、模型和单条请求，切换时保留筛选条件。
- **如实呈现缺口：** 没有活动和缺少计时用不同的标记区分，曲线不会在缺口之间凭空补值。
- **有用的对比：** 加权吞吐、P50/P90、速度分布和上下文大小散点图。
- **默认纯本地：** 不需要 API key，不调用模型，不上报遥测。增量 SQLite 索引让之后的读取很快。
- **可用于脚本：** 除交互式面板外，还能输出表格、JSON 和 CSV 报表。

> [!IMPORTANT]
> **tok/s 指的是估算的请求吞吐量**，由输出 token 数和日志时间戳计算得出，其中包含推理、预填充、网络延迟和其他等待时间。它不是纯粹的流式解码速度，也不是不同提供商之间的受控对比。计时覆盖率始终可见，没有计时的用量仍计入总数。[查看度量细节 →](docs/metrics.md)

## 快速开始

```bash
cargo install --git https://github.com/libukai/token-speed --locked
token-speed --today
```

需要 **Rust 1.92+** 和 C 编译器，SQLite 已内置。Cargo 默认安装到 `~/.cargo/bin`，请确认它在 `PATH` 中。在 macOS 上，如果自定义的编译器封装导致 SDK 或链接错误，可设置 `CC=/usr/bin/clang` 和 `CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER=/usr/bin/clang`（Intel 芯片用 `CARGO_TARGET_X86_64_APPLE_DARWIN_LINKER`）。

发布后，预编译文件和 npm 包 `@libukai/token-speed` 会附在 [Releases](https://github.com/libukai/token-speed/releases) 中。npm 上不带作用域的 `token-speed` 是[上游](https://github.com/william1010121/token-speed)项目，不含 DSH 和 Magpie 数据源。

### 从源码构建

需要 **Rust 1.92+** 和 C 编译器，SQLite 已内置。

```bash
git clone https://github.com/libukai/token-speed.git
cd token-speed
./install.sh
token-speed --today
```

安装脚本使用仓库中的锁文件构建，安装到 `~/.local/bin`，可通过 `TOKEN_SPEED_INSTALL_DIR` 修改。如有需要，把 `export PATH="$HOME/.local/bin:$PATH"` 加到 shell 配置文件中。

请使用支持真彩色的终端。推荐 **120 × 40**；**80 × 24** 会切换为紧凑布局；最小 **60 × 20**。

## 四种视图

| 视图 | 内容 |
| --- | --- |
| **Overview（总览）** | 大号吞吐卡片、活动图、最新请求和模型排名。 |
| **Timeline（时间线）** | 分开或叠加的各来源曲线、覆盖条，以及可移动的时间段检查器。 |
| **Models（模型）** | 对比表、P50/P90、速率直方图，以及输入大小与吞吐的散点图。 |
| **Requests（请求）** | 可滚动的请求明细，包含 token 数、计时质量和会话信息。 |

<details>
<summary><strong>Timeline</strong> · 准确看出曲线为什么断开</summary>

![时间线：各来源独立曲线和选中的时间段](docs/preview-timeline.png)

按 `g` 切换分开或叠加显示，用 `←` / `→` 查看某个时间段。`░` 表示没有请求，`!` 表示有用量但没有可用的计时。彩色块表示输出量，在每个来源内部单独归一化。检查器会说明选中时间段的情况。

</details>

<details>
<summary><strong>Models</strong> · 比较分布，而不只是平均值</summary>

![模型：对比表、吞吐直方图和上下文散点图](docs/preview-models.png)

选中一个模型，可查看它的 P50/P90、请求速率分布以及与上下文大小的关系。

</details>

<details>
<summary><strong>Requests</strong> · 把一个数字追溯到具体请求</summary>

![请求：可滚动记录和元数据侧栏](docs/preview-requests.png)

查看单条响应，以及用来估算吞吐的时间区间。按 `Enter` 查看详情。

</details>

```bash
token-speed                         # 最近七天，总览
token-speed --today --view timeline
token-speed --days 30 --view models
token-speed --provider claude --model opus --view requests
```

## 键盘优先，也支持鼠标

| 按键 | 作用 |
| --- | --- |
| `Tab` / `v`、`Shift-Tab`、`F1`–`F4` | 下一个 / 上一个视图，或直接跳转 |
| 鼠标 | 点击标签页、来源、时间范围、表格行（再点一次看详情）、图表时间段和 `◐` 主题按钮；滚轮滚动 |
| `1` / `2` / `3` / `4` | 全部 / Codex / Claude / DSH（有 DSH 日志时才显示） |
| `t` / `w` / `m` / `a`、`d` | 今天 / 7 天 / 30 天 / 全部；循环切换范围 |
| `T` | 切换深色 / 浅色主题 |
| `/`、`Esc` | 按模型或会话筛选；关闭 / 清除 |
| `↑` / `↓`、`j` / `k`、鼠标滚轮 | 选择行；`Home` / `End` 跳到首尾 |
| `←` / `→`、`g` | 时间线时间段 / 分开或叠加 |
| `Enter` | 请求详情 |
| `Space`、`r` | 暂停自动刷新 / 立即刷新 |
| `?`、`q` / `Ctrl-C` | 帮助 / 退出 |

后台每五秒刷新一次，可用 `--interval` 调整（最小 1 秒）。TUI 至少保留最近 200 条请求，可用 `--limit` 扩大。

## 脚本友好

```bash
# 带边框和加权总计行的纯文本表格
token-speed --table --today

# JSON 汇总（stdout 只有 JSON）
token-speed --json --days 7 > speed.json

# 今天每小时的吞吐
token-speed summary --today --group hour

# 较长时间段的模型对比
token-speed summary --days 30 --group model --provider codex

# 最近的响应，或机器可读的导出
token-speed recent --today --limit 20
token-speed recent --json --today --limit 20 > requests.json
token-speed summary --days 7 --format json > speed.json
token-speed recent --days 30 --format csv > requests.csv

# 指定时区下的日期范围（包含首尾两天）
token-speed --since 2026-10-01 --until 2026-10-08 --timezone Asia/Shanghai
```

即使在交互终端中，`--table`、`--json` 或显式的 `--format` 也会直接输出报表。不带任何参数时默认打开面板。`summary` 和 `recent` 始终输出报表；没有交互终端时，默认命令也会输出报表。`watch` 需要交互终端，打开面板。运行 `token-speed --help` 查看全部选项。

文本表格带边框，数字列右对齐；汇总报表还有一行总计，根据所有匹配的请求重新计算。缺少计时或被排除的区间显示为 `—`，这些请求仍计入用量总数。JSON 保留 `metric`、`max_gap_seconds`、`timezone`、`metadata` 和 `data` 字段，缺少的速度为 `null`；最近请求还包含 `timing_status`（`estimated`、`missing` 或 `excluded`）。

| 选项 | 作用 |
| --- | --- |
| `--provider all\|codex\|claude\|dsh\|other` | 选择来源；Magpie 的调用归入各自的 Agent，`other` 显示其余部分 |
| `--model`、`--session` | 子串筛选 |
| `--today`、`--days N`、`--all-time`、`--since`、`--until` | 时间范围 |
| `--timezone` | IANA 时区；默认使用 `TZ`（需是 IANA 名称），否则用本地时区 |
| `--view` | 面板初始视图 |
| `--theme dark\|light` | 面板配色（`TOKEN_SPEED_THEME`）；运行时按 `T` 切换 |
| `--table`、`--json` | 一次性输出文本表格或 JSON 报表，不进入 TUI |
| `--group hour\|day\|model`、`--format table\|json\|csv` | 报表分组和格式 |
| `--max-gap SECONDS` | 可用计时区间的上限，默认 300 |
| `--interval SECONDS` | 面板刷新周期，默认 5，最小 1 |
| `--codex-dir`、`--claude-dir`、`--dsh-dir`、`--magpie-dir`、`--cache` | 覆盖来源或索引位置 |
| `--refresh` | 重建缓存条目 |

## 日志只留在你的机器上

| 来源 | 默认位置 | 覆盖方式 |
| --- | --- | --- |
| Codex | `~/.codex/{sessions,archived_sessions}` | `CODEX_HOME` 或 `--codex-dir` |
| Claude Code | `~/.claude/projects` | `CLAUDE_CONFIG_DIR` 或 `--claude-dir` |
| DSH（DeepSeek Harness） | `~/.dsh/sessions`（`*.jsonl.zstd`） | `DSH_HOME` 或 `--dsh-dir`（可重复） |
| Magpie 网关（每条调用记录 `agent`） | `~/.config/magpie/usage.jsonl` | `MAGPIE_HOME` 或 `--magpie-dir`（可重复） |
| SQLite 索引 | `~/.cache/token-speed/index.sqlite3` | `XDG_CACHE_HOME` 或 `--cache` |

解析器会读取子 Agent 的日志，并对支持的用量记录去重。缓存保存 token 数、时间戳、模型名、会话 ID 和来源路径，**不保存**提示词和工具输出内容。报表可能包含会话 ID 和文件路径，分享前请脱敏。

首次运行会索引全部历史，之后只扫描文件元数据并重新解析有变化的文件。不完整的 JSONL 末行会被跳过，文件变化后再重试。各来源的格式限制见[度量与解析说明](docs/metrics.md)。

## 参与贡献

请运行 `cargo test --locked`、`cargo clippy --locked --all-targets -- -D warnings` 和 `cargo fmt --check`。测试请使用合成数据，不要提交个人的 Agent 日志。[贡献指南](CONTRIBUTING.md) · [Codex 审查配置](docs/codex-review.md) · [更新日志](CHANGELOG.md)。

维护者：[npm 打包与发布](docs/publishing.md)。

## 上游与致谢

本项目 fork 自 **[william1010121/token-speed](https://github.com/william1010121/token-speed)**（MIT），上游提供了 Codex 和 Claude Code 解析器、四个视图、SQLite 索引以及发布与 npm 打包。本 fork 新增了：

- **DSH**（DeepSeek Harness）数据源，按模型步骤计时。
- **Magpie** 网关账本数据源，统计接入网关的 Agent，并补上它们自身日志缺失的部分。
- 数据源注册表、深色 / 浅色主题和鼠标支持、Codex / Claude 速度模式，以及标题下方的实时状态行。

适合上游的改动也欢迎提交给上游。原始版权声明保留在 [LICENSE](LICENSE) 中。

基于 [Ratatui](https://ratatui.rs/) 和 [Crossterm](https://github.com/crossterm-rs/crossterm) 构建。独立社区项目，与 OpenAI 和 Anthropic 无关。

[MIT 许可证](LICENSE)。

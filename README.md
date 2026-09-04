# DocSniffer

DocSniffer 是一款跨平台（Windows / macOS / Linux）的 **本地文件搜索与内容检索** 桌面应用，基于 Tauri 2 + Rust + Tantivy 构建。它扫描本地目录、为目标文件建立全文索引，并提供基于自定义规则的匹配检测。所有数据仅保存在本地，不会上传任何内容。

## 功能特性

- **目录扫描**：递归遍历指定目录，实时反馈扫描进度（已处理数 / 总数 / 当前路径）。
- **全文索引**：对文件路径、文件名、文件内容建立 Tantivy 索引，按 BM25 相关性排序。
- **内容提取**：支持多种纯文本 / 代码文件、Office（DOCX / XLSX / PPTX）与 PDF 的文本提取；纯文本自动识别编码（含 GBK 等历史中文编码）。
- **快速搜索**：支持文件名 / 路径 / 内容检索，含高级查询语法（`path:`、`ext:`、`size:`、`mtime:`、`-term`、`OR`、引号短语）。
- **规则检测**：内置若干默认规则（正则 / 关键词），支持在界面上增删改并持久化，对文件名和 / 或文件内容进行匹配，输出命中报告。
- **增量更新**：文件监控模块（`notify`）实时监听目录变化，对发生变更的文件做增量重索引（1s 防抖），避免反复全盘扫描。
- **绿色便携**：编译为单文件可执行程序，可通过 `PORTABLE.flag` 切换数据目录（见下文）。

## 技术栈

| 层级 | 选型 |
|------|------|
| 跨平台框架 | Tauri 2（使用系统原生 WebView，无需打包浏览器内核） |
| 后端语言 | Rust |
| 全文检索引擎 | Tantivy 0.22（BM25 相关性评分、中文单字 Tokenizer） |
| 前端框架 | React 18 + TypeScript + Vite 5 |
| 文件遍历 | `walkdir` |
| 文件监控 | `notify` |
| 内容提取 | `pdf-extract`（PDF）、`calamine`（XLSX）、`zip` + 自研清理（DOCX / PPTX）、`chardetng`（编码检测） |
| 存储 | 轻量 JSON 键值存储（无第三方原生依赖） |

## 系统架构

DocSniffer 的命令层通过 Tauri IPC 与前端交互，业务核心按模块划分：

```
┌─────────────────────────── 前端 (React) ───────────────────────────┐
│  搜索页 (SearchBar + ResultList)  扫描页 (ScanControl)  规则页 (RuleManager) │
└────────────────────────────────────┬─────────────────────────────┘
                                     │  Tauri Commands (IPC)
┌────────────────────────────────────▼─────────────────────────────┐
│                       命令层 (commands/)                         │
│   scan_directory · index_status · search_files                 │
│   get_rules · save_rules · sensitive_scan                      │
└────────────────────────────────────┬─────────────────────────────┘
                                     │
┌────────────────────────────────────▼─────────────────────────────┐
│                        核心层 (core/)                            │
│  scanner   目录遍历，收集文件元数据 (path/name/ext/size/mtime)   │
│  extractor 内容提取（文本/Office/PDF，编码检测）                │
│  indexer   Tantivy 索引、CJK Tokenizer、增删改查询             │
│  searcher  查询预处理与结果组装（含 snippet 高亮）              │
│  sensitive 规则匹配检测（文件名/内容）                          │
│  watcher   文件系统监控，触发增量重索引                          │
│  storage   便携感知的 JSON 键值存储                              │
└────────────────────────────────────────────────────────────────────┘
```

## 索引设计

索引字段（Tantivy Schema）：

| 字段 | 类型 | 索引 | 存储 | 说明 |
|------|------|------|------|------|
| `id` | STRING (raw) | 是 | 是 | 文件唯一标识（路径的 SHA-256 哈希） |
| `path` | TEXT | 是 | 是 | 文件完整路径 |
| `name` | TEXT | 是 | 是 | 文件名 |
| `content` | TEXT | 是 | 否 | 文件内容（用 `zh` Tokenizer，仅索引不存储） |
| `ext` | STRING (raw) | 是 | 是 | 文件扩展名 |
| `size` | U64 (fast) | 是 | 是 | 文件字节数 |
| `mtime` | U64 (fast) | 是 | 是 | 修改时间戳（秒） |

> `content` 刻意不存储以保持索引体积；结果中的摘要（snippet）通过重新读取文件文本生成。中文采用自研单字 Tokenizer，无需引入外部分词库。

## 查询语法

| 语法 | 示例 | 说明 |
|------|------|------|
| `path:` | `path:reports` | 限定搜索路径 |
| `ext:` | `ext:pdf` | 限定文件扩展名 |
| `size:` | `size:>10mb` | 按文件大小过滤（`b/kb/mb/gb`，自动换算） |
| `mtime:` | `mtime:>2026-01-01` | 按修改时间过滤（自动换算为时间戳） |
| `-term` | `-draft` | 排除包含该词的结果 |
| `OR` | `报告 OR 文档` | 或逻辑 |
| 引号 | `"项目计划书"` | 短语精确匹配 |

## 数据目录与便携模式

应用数据（索引、规则、配置）默认存放在用户数据目录下的 `DocSniffer/`（macOS / Linux 为用户 `data_local_dir`，Windows 为 `%LOCALAPPDATA%`）。

若要“单文件随行”：在与可执行文件同一级目录放置一个空文件 `PORTABLE.flag`，则所有运行时数据（索引、规则、配置、日志）都会写入 exe 所在目录下的 `Data/` 子目录，宿主机不再遗留应用数据。

## 构建与运行

前置要求：Node.js / pnpm（或 npm）、Rust 工具链、Tauri 2 所需系统依赖。

```bash
# 安装前端依赖
pnpm install

# 开发调试（热更新）
npx tauri dev

# 生产构建（跳过安装包，仅生成 release 可执行文件）
npx tauri build --no-bundle
```

产物位于：

- Windows：`src-tauri/target/release/docsniffer.exe`
- macOS：`src-tauri/target/release/docsniffer`（或 `.app`，取决于打包配置）
- Linux：`src-tauri/target/release/docsniffer`

> Windows 下 release 构建通过 `.cargo/config.toml` 启用 `+crt-static` 静态链接，避免依赖 VC++ 运行库，保证单文件可直接运行。macOS / Linux 使用系统自带 WebKit，无需打包浏览器内核。

## 目录结构

```
DocSniffer/
├── src/                            # 前端 (React + Vite + TypeScript)
│   ├── main.tsx
│   ├── App.tsx
│   ├── styles.css
│   ├── lib/api.ts                  # Tauri 命令 / 事件封装
│   └── components/
│       ├── SearchBar.tsx
│       ├── ResultList.tsx
│       ├── ScanControl.tsx
│       └── RuleManager.tsx
├── resources/
│   └── rules/default_rules.json    # 内置默认规则（由 rust-embed 内嵌进二进制）
├── src-tauri/                      # Tauri 后端 (Rust)
│   ├── src/
│   │   ├── main.rs                 # 二进制入口
│   │   ├── lib.rs                  # 注册命令并启动
│   │   ├── commands/               # IPC 命令层
│   │   │   ├── mod.rs
│   │   │   ├── scan.rs
│   │   │   ├── search.rs
│   │   │   └── sensitive.rs
│   │   └── core/                   # 业务核心
│   │       ├── mod.rs
│   │       ├── scanner/            # 目录遍历
│   │       │   ├── mod.rs
│   │       │   └── walker.rs
│   │       ├── extractor/          # 内容提取
│   │       │   ├── mod.rs
│   │       │   ├── text.rs
│   │       │   └── office.rs
│   │       ├── indexer/            # Tantivy 索引
│   │       │   ├── mod.rs
│   │       │   ├── schema.rs
│   │       │   └── writer.rs
│   │       ├── searcher/           # 查询引擎
│   │       │   ├── mod.rs
│   │       │   └── parser.rs
│   │       ├── sensitive/          # 规则检测
│   │       │   ├── mod.rs
│   │       │   ├── rules.rs
│   │       │   └── scanner.rs
│   │       ├── watcher/            # 文件监控
│   │       │   └── mod.rs
│   │       └── storage/            # JSON 存储
│   │           └── mod.rs
│   ├── Cargo.toml
│   ├── build.rs
│   ├── tauri.conf.json
│   └── icons/                      # 应用图标
├── vite.config.ts
├── package.json
└── README.md
```

## 许可

本项目仅供学习与个人本地使用。请遵守所在地区法律法规及第三方依赖的开源许可协议。

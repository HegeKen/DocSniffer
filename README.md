# DocSniffer

DocSniffer 是一款跨平台（Windows / macOS / Linux）的 **本地文件搜索与内容检索** 桌面应用，基于 Tauri 2 + Rust + Tantivy 构建。它扫描本地目录、为目标文件建立全文索引，并提供基于自定义规则的匹配检测。所有数据仅保存在本地，不会上传任何内容。

当前版本 **v0.3.0**，变更记录见 [CHANGELOG.md](CHANGELOG.md)。

> **旧系统用户请注意**：自 v0.3.0 起，本仓库最低支持 Windows 10（1803 及以上）。仍在使用 Windows 7 / XP 等系统的用户，请改用向下兼容版本 **[DocSnifferLegacy](https://github.com/HegeKen/DocSnifferLegacy)** —— 与本项目保持一致的批次模型、规则检测语义与界面布局，面向 .NET Framework 4.0 的 WinForms 实现（详见[相关项目](#相关项目)）。

## 系统要求

| 平台 | 要求 |
|------|------|
| Windows | Windows 10（1803 及以上），依赖 WebView2 运行时（Win10 / Win11 通常已内置） |
| macOS | macOS 10.15（Catalina）及以上 |
| Linux | 需系统安装 WebKitGTK（Tauri 2 要求 `webkit2gtk-4.1`） |

> 界面依赖 Tauri 2 的系统 WebView 渲染层（WebView2 / WebKitGTK）。Windows 7 及更早系统不在支持范围内，请使用 [DocSnifferLegacy](https://github.com/HegeKen/DocSnifferLegacy)。

## 功能特性

- **目录扫描**：递归遍历指定目录，实时反馈扫描进度（已处理数 / 总数 / 当前路径）；被跳过或索引失败的条目汇总为告警列表在界面展示，不再静默丢弃。默认排除各平台系统目录（Linux 的 `/proc`、`/sys` 等，Windows 的系统盘关键目录与回收站 / 卷影目录，macOS 的 `/System`、`/Applications` 等）。
- **全文索引**：对文件路径、文件名、文件内容建立 Tantivy 索引，按 BM25 相关性排序。中文使用自研 CJK 流式单字分词器，无需引入外部分词库。
- **内容提取**：支持多种纯文本 / 代码文件（自动识别编码，含 GBK、GB18030 等历史中文编码）、Office（DOCX / XLSX / PPTX 及 WPS `.wps`、`.dps`、`.et`）与 PDF 的文本提取。
- **快速搜索**：文件名 / 路径 / 内容统一检索，支持高级查询语法（`path:`、`ext:`、`size:`、`mtime:`、`-term`、`OR`、引号短语），结果含命中摘要与关键词高亮。
- **敏感信息检测**：内置若干默认规则（正则 / 关键词），支持在界面上增删改并持久化，可作用于文件名和 / 或文件内容，输出带风险等级的命中报告。
- **批次化索引管理**：每次扫描生成一个索引批次，可查看各批次实时文档数、重新扫描更新（变更文件重索引、新增文件入库、已删除文件出库）、删除单个批次或一键清空全部索引；搜索可限定到指定批次。
- **大文件内存防护**：单文件提取上限 50MB、单文档索引内容上限 200 万字符、压缩格式解压量校验（防 zip bomb），超大文件降级为“仅按元数据索引”，不会拖垮进程。
- **绿色便携**：编译为单文件可执行程序，可通过 `PORTABLE.flag` 切换数据目录（见下文）。

## 技术栈

| 层级 | 选型 |
|------|------|
| 跨平台框架 | Tauri 2（桌面版，使用系统原生 WebView，无需打包浏览器内核） |
| 后端语言 | Rust |
| 全文检索引擎 | Tantivy 0.22（BM25 相关性评分、中文单字 Tokenizer） |
| 前端框架 | React 18 + TypeScript + Vite 5 |
| 文件遍历 | `walkdir` |
| 文件监控 | `notify`（模块已就绪，当前由“更新批次”触发增量重索引） |
| 内容提取 | `pdf-extract`（PDF）、`calamine`（XLSX / WPS `.et`）、`office_oxide`（WPS `.wps` / `.dps` OLE）、`zip` + 自研清理（DOCX / PPTX）、`chardetng`（编码检测） |
| 存储 | 轻量 JSON 键值存储（原子写入，无第三方原生依赖） |

## 系统架构

前端通过 Tauri IPC 调用命令层，命令层驱动同一套业务核心：

```
┌─────────────────────────── 前端 (React) ────────────────────────────┐
│  搜索 (SearchBar + ResultList)    扫描 (ScanControl)                 │
│  敏感检测 (RuleManager)           索引管理 (BatchManager)            │
│               api.ts 封装 Tauri 命令调用与事件监听                    │
└────────────────────────────────┬────────────────────────────────────┘
                                 │  Tauri Commands (IPC)
┌────────────────────────────────▼────────────────────────────────────┐
│      命令层 (commands/)                                             │
│  scan_directory · index_status · list_batches · delete_batch ·      │
│  update_batch · clear_all_index · search_files · get_rules ...      │
└────────────────────────────────┬────────────────────────────────────┘
                                 │
┌────────────────────────────────▼────────────────────────────────────┐
│                        核心层 (core/)                               │
│  scanner   目录遍历，收集文件元数据 (path/name/ext/size/mtime)       │
│  extractor 内容提取（文本 / Office / PDF，编码检测）                 │
│  indexer   Tantivy 索引、CJK Tokenizer、增删改查、按批次统计         │
│  searcher  查询预处理与结果组装（含 snippet 高亮）                   │
│  sensitive 规则匹配检测（文件名 / 内容）                             │
│  batch     批次元数据（一次扫描 = 一个批次）                         │
│  watcher   文件系统监控（notify，模块就绪，当前由手动更新批次触发）   │
│  storage   便携感知的 JSON 键值存储                                  │
└─────────────────────────────────────────────────────────────────────┘
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
| `batch_id` | STRING (raw) | 是 | 是 | 所属导入批次 ID（按批次统计 / 删除） |

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

应用数据（索引、批次元数据、规则、配置）默认存放在用户数据目录下的 `DocSniffer/`（macOS / Linux 为用户 `data_local_dir`，Windows 为 `%LOCALAPPDATA%`）。

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

> v0.3.0 起不再提供无头服务器模式（`docsniffer-server`）与 Windows 7 构建链路，本项目只构建桌面版；旧系统请使用 [DocSnifferLegacy](https://github.com/HegeKen/DocSnifferLegacy)。

## 目录结构

```
DocSniffer/
├── src/                            # 前端 (React + Vite + TypeScript)
│   ├── main.tsx
│   ├── App.tsx                     # 四标签页外壳（搜索 / 扫描 / 敏感检测 / 索引管理）
│   ├── ErrorBoundary.tsx           # 按 Tab 重置的界面错误边界
│   ├── styles.css
│   ├── lib/api.ts                  # Tauri 命令 / 事件封装
│   └── components/
│       ├── SearchBar.tsx
│       ├── ResultList.tsx
│       ├── ScanControl.tsx
│       ├── RuleManager.tsx
│       └── BatchManager.tsx
├── resources/
│   └── rules/default_rules.json    # 内置默认规则（由 rust-embed 内嵌进二进制）
├── src-tauri/                      # Tauri 后端 (Rust)
│   ├── src/
│   │   ├── main.rs                 # 桌面版二进制入口
│   │   ├── lib.rs                  # 注册命令并启动
│   │   ├── commands/               # IPC 命令层
│   │   │   ├── mod.rs
│   │   │   ├── scan.rs             # 扫描 / 批次 / 索引状态
│   │   │   ├── search.rs
│   │   │   └── sensitive.rs
│   │   └── core/                   # 业务核心
│   │       ├── mod.rs              # 数据目录解析（便携模式）
│   │       ├── state.rs            # 共享应用状态 AppState
│   │       ├── batch.rs            # 批次元数据与 ID 生成
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
│   │       ├── watcher/            # 文件监控（预留）
│   │       │   └── mod.rs
│   │       └── storage/            # JSON 存储
│   │           └── mod.rs
│   ├── capabilities/default.json   # Tauri 权限配置
│   ├── Cargo.toml
│   ├── build.rs
│   ├── tauri.conf.json
│   └── icons/                      # 应用图标
├── vite.config.ts
├── package.json
├── CHANGELOG.md
└── README.md
```

## 相关项目

| 项目 | 定位 | 运行环境 | 技术栈 |
|------|------|----------|--------|
| **DocSniffer**（本仓库） | 现代版主线，功能与性能持续演进 | Windows 10（1803+）/ macOS 10.15+ / Linux | Tauri 2 + Rust + Tantivy |
| **[DocSnifferLegacy](https://github.com/HegeKen/DocSnifferLegacy)** | 向下兼容的遗留系统版，承接 Windows 7 / XP 等旧机 | Windows XP SP3 / Vista / 7 / 8 / 10 / 11（需 .NET Framework 4.0） | .NET Framework 4.0 WinForms + Lucene.NET |

两者刻意保持一致的使用语义：**批次模型**（一次扫描一个批次，支持按批次搜索 / 更新 / 删除 / 清空）、**规则检测**（名称 + `regex`/`keyword` + 风险等级 + 作用范围）与**界面布局**（搜索 / 扫描 / 敏感检测 / 索引管理 四页）。数据目录相互独立（本仓库使用用户数据目录下的 `DocSniffer/`，DocSnifferLegacy 使用 `%APPDATA%\DocSnifferLegacy\`），可在一台机器上并存。

> 自 v0.3.0 起，Windows 7 及更早系统的支持由 DocSnifferLegacy 承接，本仓库不再维护对应构建链路。

## 许可

本项目仅供学习与个人本地使用。请遵守所在地区法律法规及第三方依赖的开源许可协议。

# DocSniffer

DocSniffer 是一款跨平台（Windows / macOS / Linux）的 **本地文件搜索与内容检索** 桌面应用，基于 Tauri 2 + Rust + Tantivy 构建。它扫描本地目录、为目标文件建立全文索引，并提供基于自定义规则的匹配检测。所有数据仅保存在本地，不会上传任何内容。

除图形界面外，项目还提供 **服务器模式**（`docsniffer-server`）：一个无 GUI / WebView 依赖的本地 HTTP 服务，自带内嵌 Web 界面，是 Windows 7 的官方支持途径（也可在任何平台上作无头模式使用）。

## 系统要求

| 平台 | 桌面版（Tauri GUI） | 服务器模式（docsniffer-server） |
|------|---------------------|--------------------------------|
| Windows | Windows 10（1803 及以上） | **Windows 7 SP1 及以上**（含 8 / 8.1 / 10 / 11），需浏览器访问界面 |
| macOS | macOS 10.15（Catalina）及以上 | 同左（无头运行则无额外要求） |
| Linux | 需系统安装 WebKitGTK（Tauri 2 要求 `webkit2gtk-4.1`） | 无额外要求 |

> - 桌面版界面依赖 Tauri 2 的 WebView2 / WebKitGTK 渲染层，仅支持 Windows 10+。
> - 服务器模式界面在浏览器中渲染，**Win7 上最后可安装的 Chrome 109 / Firefox ESR 115 均满足要求**（界面按 Chrome 87+ / Firefox 78+ 构建）；IE11 不受支持。
> - Windows Vista / XP 无法支持：Rust 工具链与上述浏览器的最低系统要求均为 Windows 7。

## 功能特性

- **目录扫描**：递归遍历指定目录，实时反馈扫描进度（已处理数 / 总数 / 当前路径）。
- **全文索引**：对文件路径、文件名、文件内容建立 Tantivy 索引，按 BM25 相关性排序。
- **内容提取**：支持多种纯文本 / 代码文件、Office（DOCX / XLSX / PPTX / WPS `.wps`、`.dps`、`.et`）与 PDF 的文本提取；纯文本自动识别编码（含 GBK 等历史中文编码）。
- **快速搜索**：支持文件名 / 路径 / 内容检索，含高级查询语法（`path:`、`ext:`、`size:`、`mtime:`、`-term`、`OR`、引号短语）。
- **规则检测**：内置若干默认规则（正则 / 关键词），支持在界面上增删改并持久化，对文件名和 / 或文件内容进行匹配，输出命中报告。
- **增量更新**：文件监控模块（`notify`）实时监听目录变化，对发生变更的文件做增量重索引（1s 防抖），避免反复全盘扫描。
- **服务器模式**：`docsniffer-server` 复用同一 Rust 核心，通过本地 HTTP 提供 REST API + 内嵌 Web 界面，无任何 GUI 依赖 —— **Windows 7 的支持途径**，亦可作无头 / 远程模式。
- **绿色便携**：编译为单文件可执行程序，可通过 `PORTABLE.flag` 切换数据目录（见下文）。

## 技术栈

| 层级 | 选型 |
|------|------|
| 跨平台框架 | Tauri 2（桌面版，使用系统原生 WebView，无需打包浏览器内核） |
| 服务器模式 | `tiny_http`（纯 Rust std TCP，无 GUI / WebView 依赖，Win7 可用） |
| 后端语言 | Rust |
| 全文检索引擎 | Tantivy 0.22（BM25 相关性评分、中文单字 Tokenizer） |
| 前端框架 | React 18 + TypeScript + Vite 5 |
| 文件遍历 | `walkdir` |
| 文件监控 | `notify` |
| 内容提取 | `pdf-extract`（PDF）、`calamine`（XLSX / WPS `.et`）、`office_oxide`（WPS `.wps` / `.dps`  OLE）、`zip` + 自研清理（DOCX / PPTX）、`chardetng`（编码检测） |
| 存储 | 轻量 JSON 键值存储（无第三方原生依赖） |

## 系统架构

桌面版通过 Tauri IPC 与前端交互，服务器模式通过本地 HTTP（`server/` 模块，接口与命令一一对应）暴露同一套业务核心：

```
┌─────────────────────────── 前端 (React) ───────────────────────────┐
│  搜索页 (SearchBar + ResultList)  扫描页 (ScanControl)  规则页 (RuleManager) │
│     api.ts 自动探测运行环境：Tauri IPC 或 HTTP 适配（双模式复用）       │
└───────────────┬────────────────────────────────┬───────────────────┘
                │  Tauri Commands (IPC)          │  Local HTTP (docsniffer-server)
┌───────────────▼───────────────┐┌───────────────▼───────────────────┐
│      命令层 (commands/)       ││      服务器模块 (server/)           │
│  scan_directory · index_status││  /api/scan_directory · /api/rules  │
│  search_files · get_rules ... ││  + 内嵌前端静态资源 (rust-embed)     │
└───────────────┬───────────────┘└───────────────┬───────────────────┘
                │                                │
┌───────────────▼────────────────────────────────▼──────────────────┐
│                        核心层 (core/)                            │
│  scanner   目录遍历，收集文件元数据 (path/name/ext/size/mtime)   │
│  extractor 内容提取（文本/Office/PDF，编码检测）                │
│  indexer   Tantivy 索引、CJK Tokenizer、增删改查询             │
│  searcher  查询预处理与结果组装（含 snippet 高亮）              │
│  sensitive 规则匹配检测（文件名/内容）                          │
│  watcher   文件系统监控，触发增量重索引                          │
│  storage   便携感知的 JSON 键值存储                              │
└───────────────────────────────────────────────────────────────────┘
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

若要“单文件随行”：在与可执行文件同一级目录放置一个空文件 `PORTABLE.flag`，则所有运行时数据（索引、规则、配置、日志）都会写入 exe 所在目录下的 `Data/` 子目录，宿主机不再遗留应用数据。该机制对桌面版与服务器模式同样生效。

## Windows 7 支持（服务器模式）

Tauri 2 官方已放弃 Windows 7（其 WebView2 渲染层要求 Windows 10+），Rust 自 1.76 起也将 Win7 移出了标准 `*-pc-windows-msvc` 目标。为继续支持 Windows 7，DocSniffer 提供 **服务器模式**：

```
┌──────────────┐   浏览器访问 http://127.0.0.1:8765/    ┌──────────────────────┐
│ Win7 + 浏览器 │ ────────────────────────────────────► │ docsniffer-server.exe │
│ Chrome 109 /  │ ◄──────────── 内嵌 Web 界面 + JSON API │  （无 GUI 依赖，       │
│ Firefox 115   │                                       │   纯 std 网络）       │
└──────────────┘                                       └──────────────────────┘
```

- 单文件 `docsniffer-server.exe`，与桌面版共享同一 Rust 核心（扫描 / 索引 / 搜索 / 规则检测功能完全一致），前端界面直接内嵌在二进制里。
- 默认仅监听 `127.0.0.1:8765`（本机访问，无鉴权）；`--host 0.0.0.0` 可改为局域网访问，但请自行评估风险。
- 索引 / 规则数据目录与桌面版一致，便携模式（`PORTABLE.flag`）同样适用。

### 使用方法

```bat
docsniffer-server.exe                 :: 默认 http://127.0.0.1:8765/
docsniffer-server.exe --port 9000     :: 指定端口
docsniffer-server.exe --open          :: 启动后自动打开默认浏览器
```

启动后在浏览器打开提示的地址即可；界面与桌面版完全相同。命令行参数：`--port <n>`、`--host <ip>`、`--open`、`--help`。

### 面向 Windows 7 构建

在任意装有 **VS Build Tools（MSVC 链接器）的 Windows 10/11 机器**上交叉构建，产物拷贝到 Win7 直接运行：

```bash
rustup toolchain install nightly

cd src-tauri
cargo +nightly build --release --no-default-features --bin docsniffer-server \
      -Z build-std=std,panic_abort --target x86_64-win7-windows-msvc
# 产物：src-tauri/target/x86_64-win7-windows-msvc/release/docsniffer-server.exe
```

要点：

- `x86_64-win7-windows-msvc` 是 Rust 的 Tier-3 目标，std 以 Win7 为最低系统编译，需 nightly + `-Z build-std`（详见 [rustc 平台支持文档](https://doc.rust-lang.org/rustc/platform-support/win7-windows-msvc.html)）。
- `--no-default-features` 关闭 `tauri-app` feature：不编译 Tauri / WebView 层，这正是 Win7 兼容的关键。
- 仓库的 `.cargo/config.toml` 已对所有 Windows 目标启用 `+crt-static`，产物为静态链接单文件，**Win7 无需安装 UCRT / VC++ 运行库**。
- 如需 32 位 Win7，将目标换为 `i686-win7-windows-msvc` 即可。
- 构建 `docsniffer-server` 前需先执行一次 `pnpm build`（release 构建会把 `dist/` 前端产物嵌入二进制）。

### 常见问题

- **能否让桌面版（Tauri GUI）跑在 Win7 上？** 网上存在捆绑旧版 WebView2 109 + 锁旧工具链的非官方改造方案，但不受官方支持、无法跟进安全更新，本项目不采用；服务器模式是唯一受支持的 Win7 途径。
- **Vista / XP？** 不支持。Rust 工具链与可用浏览器的最低系统要求均为 Windows 7。

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

- Windows：`src-tauri/target/release/docsniffer.exe`（桌面版）、`docsniffer-server.exe`（服务器模式）
- macOS：`src-tauri/target/release/docsniffer`（或 `.app`，取决于打包配置）
- Linux：`src-tauri/target/release/docsniffer`

> Windows 下 release 构建通过 `.cargo/config.toml` 启用 `+crt-static` 静态链接，避免依赖 VC++ 运行库，保证单文件可直接运行。macOS / Linux 使用系统自带 WebKit，无需打包浏览器内核。

### 单独构建服务器模式（任意平台）

服务器模式不依赖 Tauri，可随时单独构建（Windows 7 的构建方法见上文专属章节）：

```bash
pnpm build                                   # 生成 dist/，release 构建会将其嵌入二进制
cd src-tauri
cargo build --release --no-default-features --bin docsniffer-server
./target/release/docsniffer-server --open    # 浏览器访问 http://127.0.0.1:8765/
```

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
│   │   ├── main.rs                 # 桌面版二进制入口
│   │   ├── bin/
│   │   │   └── docsniffer-server.rs# 服务器模式二进制入口
│   │   ├── lib.rs                  # 注册命令并启动（tauri-app feature 门控）
│   │   ├── commands/               # IPC 命令层（桌面版）
│   │   │   ├── mod.rs
│   │   │   ├── scan.rs
│   │   │   ├── search.rs
│   │   │   └── sensitive.rs
│   │   ├── server/                 # 服务器模式（本地 HTTP API + 内嵌前端）
│   │   │   ├── mod.rs
│   │   │   └── api.rs
│   │   └── core/                   # 业务核心（桌面版 / 服务器模式共用）
│   │       ├── mod.rs
│   │       ├── state.rs            # 共享应用状态 AppState
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

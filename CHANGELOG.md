# Changelog

本项目所有显著变更将记录在本文件中。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，版本号遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

## [0.4.0] - 2026-09-22

### 新增

- **索引存储位置可自定义并支持迁移**：「索引管理」页新增「索引存储位置」区块，可查看当前索引目录、在系统资源管理器中打开，或选择新目录并把现有索引迁移过去（`set_index_dir`：刷盘 → 递归复制 → 打开新索引 → 热替换 → 清理旧目录 → 持久化位置）。位置保存在本地存储的 `index_dir` 键，重启后自动沿用；索引不再固定于数据目录下的 `index`。
- **首页索引目录可点击打开**：状态栏中的索引目录路径变为可点击项，点击后在系统资源管理器中定位该目录（`open_index_dir`；macOS 使用 `open`、Windows 使用 `explorer`、Linux 使用 `xdg-open`）。
- **搜索结果可用默认程序打开**：搜索结果的每一条变为可点击项，点击后交给系统默认程序打开对应文件（`open_path`；文件已被移动或删除时在结果上方给出提示）。

### 变更

- **「索引管理」更名为「设置」**：标签与页面标题统一为「设置」，索引批次管理与索引存储位置收纳于此。
- **索引句柄支持热替换**：`AppState` 中的索引改由 `RwLock<Arc<IndexManager>>` 持有，迁移后无需重启即可切换到新目录；`index_status` 返回的 `index_dir` 改为真实索引目录而非数据目录。
- **新增依赖 `tauri-plugin-dialog`**：用于原生文件选择与确认弹窗，能力声明相应增加 `dialog:default`。

### 修复

- **索引批次「删除」「更新」点击无反应**：二者使用 `window.confirm` 做二次确认，而 macOS 上的 `wry 0.55.1` 未实现 `webView:runJavaScriptConfirmPanelWithMessage:initiatedByFrame:completionHandler:`，`confirm()` 静默返回 `false` 且不弹窗，处理逻辑随即提前返回。现改用 `tauri-plugin-dialog` 的原生 `ask()` 弹窗，删除、更新与「一键清除全部索引」的确认流程均恢复正常。

## [0.3.0] - 2026-09-16

### 修复

- **扫描/建索引频繁内存溢出（OOM）**：根因是自定义 CJK 分词器在分词前为整个文档预生成 `Vec<Token>`，且每个汉字对应一个独立堆分配的 `String`——实测 40MB 中文文件（约 1400 万字符）仅分词阶段峰值即达约 897MB，叠加 Tantivy 内存后扫描大文件或中文密集目录时频繁 OOM（32 位 Windows 7 构建尤其严重）。现改为**流式分词**（全程仅保留一个可复用 Token，峰值降为常数级）；实测 40MB 中文 + 30MB 英文全链路扫描峰值约 169MB。
- **单文档索引内容上限**：单个文件送入索引器的内容按 UTF-8 边界截断至 200 万字符（约 6MB 中文），超出部分仅不建全文索引（文件仍按元数据索引），与分词器改造共同保证任意单个文件的索引内存占用有确定上界。
- **压缩文档解压放大防护**：DOCX/PPTX/XLSX 在解析前先依据 ZIP 中央目录汇总解压后总大小，超过 100MB 直接跳过（防 zip bomb）；PDF 与 WPS OLE 旧格式（`.wps`/`.dps`）的文件大小闸门收紧至 20MB；XLSX 文本累积设 50MB 上限，不再因 `calamine` 整表驻留 + 二次拼接成倍放大内存。
- **索引写入堆预算下调**：Tantivy writer 堆预算由 150MB 降至 64MB，降低 32 位 Windows 7 构建的地址空间压力（段刷盘更频繁但单段更小）。

### 移除

- **移除 Windows 7 及以下系统的支持**：Win7 用户的替代方案已由 [DocSnifferLegacy](https://github.com/HegeKen/DocSnifferLegacy) 项目承接并上线，本仓库不再维护 Win7 构建链路（删除 `scripts/build-win7.ps1`、`x86_64-win7-windows-msvc` 构建方式及相关文档），最低系统要求回归 Windows 10（1803 及以上）。
- **移除服务器模式（`docsniffer-server`）**：该模式原为 Win7 支持途径，随 Win7 支持一并移除——删除无头 HTTP 服务（`server/` 模块、`tiny_http` 等依赖）与 `--no-default-features` 无头构建；前端 `api.ts` 移除 HTTP 双通道适配，仅保留 Tauri IPC。

## [0.2.0] - 2026-09-13

### 新增

- **扫描报告**：`scan_directory`（Tauri 命令与 `/api/scan_directory`）完成后返回 `ScanReport { indexed, warnings }`，其中 `warnings` 为被跳过或索引失败的条目列表（如权限拒绝、读取错误），上限 100 条；前端扫描页展示警告列表（最多显示前 10 条）。
- **服务器绑定策略**：`docsniffer-server` 默认仅允许绑定回环地址（`127.0.0.1` / `localhost` / `::1`）；新增 `--allow-remote` CLI 开关，显式授权后才允许绑定非回环地址（启动时打印无认证风险警告）。
- **服务器安全响应头**：所有 HTTP 响应附加 `Content-Security-Policy`、`X-Content-Type-Options: nosniff`、`X-Frame-Options: DENY`、`Referrer-Policy: no-referrer`。
- **优雅关闭**：服务器捕获 SIGINT / SIGTERM（Windows 使用控制台事件处理器），Ctrl+C 或终止信号后正常退出，确保索引 commit 落盘。
- **跨平台系统目录排除**：扫描排除目录按平台动态生成（Linux：`/proc`、`/sys`、`/dev` 等；Windows：读取 `SystemRoot`、`ProgramFiles` 等环境变量，并排除回收站与卷影目录），不再硬编码 macOS 路径。

### 变更

- **扫描结果契约**：`scan_directory` 返回值由 `number`（索引数）变更为 `ScanReport` 对象，前端与 API 消费方需同步适配。
- **批次 ID 生成**：由毫秒时间戳改为 `纳秒时间戳（hex）+ 进程内递增序号`，消除快速连续操作下的碰撞，且不可按时间枚举。
- **正则编译缓存**：敏感规则与 Office 提取器中的正则表达式改为全局缓存编译结果，避免逐文件重复编译。
- **Tauri CSP 启用**：`tauri.conf.json` 的 `csp` 由 `null` 改为严格策略（`script-src 'self'`、`object-src 'none'` 等）。

### 修复

- **存储原子写入**：JSON 存储改为「临时文件 + `rename` 原子替换」，进程崩溃不再产生半截损坏的存储文件。
- **批次列表并发竞态**：批次增删改的读-改-写全程持有存储层写锁，并发请求不再互相覆盖丢失数据。
- **内存耗尽防护**：内容提取链路新增 50MB 单文件上限（文本 / DOCX / PPTX 解压量均有界读取），超大文件跳过提取而非拖垮进程。
- **扫描错误不再静默**：目录遍历错误（权限、IO）逐条收集进扫描报告；文件索引失败与索引 commit 失败真实向上传播，不再被丢弃。
- **非法正则显式报错**：无效正则表达式不再被静默忽略，返回明确的错误信息（HTTP 400 / Tauri Err）。
- **UTF-8 安全切片**：敏感信息扫描的结果摘录（snippet）按字符边界对齐截取，多字节字符不再导致 panic。
- **ErrorBoundary 按 Tab 重置**：界面错误边界以当前 Tab 为 key，切页后自动清除崩溃状态，单个页面出错不再卡死整个应用。

### 移除

- 移除生产代码中的 `[DEBUG]` 调试输出。
- 移除 HTTP 服务器的 `Access-Control-Allow-Origin: *` 通配 CORS 头——内嵌界面为同源访问，跨站调用被浏览器同源策略拦截。

### 安全

- **未授权远程访问缓解**：默认回环绑定 + 显式 `--allow-remote` 开关 + 移除通配 CORS + CSP 启用，`docsniffer-server` 模式下同网络攻击者无法再匿名扫描本机文件、读取文件内容或删除索引。

## [0.1.0] - 2026-09-07

### 新增

- **核心功能**（`93f141b`，2026-09-04）：
  - 目录递归扫描，实时进度反馈（已处理数 / 总数 / 当前路径）。
  - 基于 Tantivy 0.22 的全文索引（路径 / 文件名 / 内容），BM25 相关性评分，中文单字分词。
  - 内容提取：多种纯文本与代码文件（自动识别编码，含 GBK 等历史中文编码）、DOCX / XLSX / PPTX、PDF。
  - 快速搜索：支持高级查询语法（`path:`、`ext:`、`size:`、`mtime:`、`-term` 排除、`OR`、引号短语）。
  - 规则检测：内置默认规则（正则 / 关键词），界面增删改并持久化，输出命中报告。
  - 桌面应用（Tauri 2 + React 18 + TypeScript + Vite 5）。
- **WPS 文件支持**（`d47df8e`，2026-09-04）：新增 WPS 格式 `.wps`（文档）、`.dps`（演示）、`.et`（表格）的文本提取。
- **服务器模式**（`3f8f3d0`，2026-09-04）：
  - `docsniffer-server` 无头 HTTP 服务：`tiny_http` 实现，无 GUI / WebView 依赖，复用同一 Rust 核心。
  - REST API + `rust-embed` 内嵌 Web 界面，浏览器访问（Chrome 87+ / Firefox 78+）。
  - **Windows 7 SP1 的官方支持途径**（桌面版 WebView 仅支持 Windows 10+）。
- **索引批次管理**（`8a29247`，2026-09-07）：
  - 扫描按批次归组，界面可视化管理批次（查看文档数、删除、重建批次索引）。
  - 搜索支持按批次过滤。
- **绿色便携模式**：单文件可执行程序，通过 exe 同级 `PORTABLE.flag` 将数据目录切换至本地 `Data/`，不写注册表 / 系统目录。

### 构建

- Win7 构建链接问题修复与 crate 类型优化（`c2e392d`，2026-09-05）；新增 Win7 构建脚本 `scripts/build-win7.ps1`。
- 包管理器迁移至 pnpm，移除 `package-lock.json`（`f6fb6bb`，2026-09-07）。

[0.3.0]: https://github.com/HegeKen/DocSniffer/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/HegeKen/DocSniffer/releases/tag/v0.2.0
[0.1.0]: https://github.com/HegeKen/DocSniffer/releases/tag/v0.1.0

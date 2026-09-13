# Changelog

本项目所有显著变更将记录在本文件中。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，版本号遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

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

[0.2.0]: https://github.com/HegeKen/DocSniffer/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/HegeKen/DocSniffer/releases/tag/v0.1.0

# WowType Windows 版开发笔记

## 构建环境（重要）

- **Windows 侧 GNU 工具链损坏**：rustup 自带 dlltool 报 "dlltool could not create import library... CreateProcess"，无法生成 .lib。
- **解法：WSL Ubuntu-24.04 交叉编译**：
  ```bash
  apt install mingw-w64
  # cargo config 用 rsproxy.cn sparse 源
  rustup target add x86_64-pc-windows-gnu
  ```
  linker = `x86_64-w64-mingw32-gcc`
- Rust 安装镜像 TLS 问题：rsproxy/ustc 失败时用 `RUSTUP_DIST_SERVER=https://static.rust-lang.org`。
- GNU 目标产物需要 `libwinpthread-1.dll` 随 exe/dll 分发（MinGW 运行时）。

## 性能数据（16GB 内存机器实测）

| 词库 | 词条数 | 文件大小 | 运行时内存 | 加载耗时 |
|------|--------|---------|-----------|---------|
| 完整 | 929,410 | 27.4 MB | ~509 MB | ~2.7 s |
| 完整无LM | 929,410 | - | ~433 MB | ~3.2 s |
| 中等 | 346,312 | 8.5 MB | ~230 MB | <1 s |
| 精简 | 96,312 | 1.9 MB | ~121 MB | ~0.3 s |

结论：词典结构内存 ≈ 文件大小 × 16（引擎把词条展开成结构化数据）。LM 约占 76 MB（×2）。

## 候选数权衡

- `engine_set_max_candidates(5)`：内存/性能最优，但同音字候选太少，生僻字找不到 → 用户强烈不满。
- `engine_set_max_candidates(100)` + page_size 9：满足"生僻字候选"需求，代价是每键计算量上升（可接受）。
- 词库必须保留**全部单字**（4.6 万）：生僻字输入靠拼音 + 翻页。

## TSF 注册机制

- `Win+Space` 列表 = TSF 语言配置文件（`ITfInputProcessorProfiles::AddLanguageProfile` 添加）。
- 注册表三处：
  - `HKLM\SOFTWARE\Classes\CLSID\{CLSID}\InprocServer32`（DLL 路径）
  - `HKLM\SOFTWARE\Microsoft\CTF\TIP\{CLSID}\LanguageProfile\0x00000804\{GUID_PROFILE}`（Enable=1）
  - `HKLM\SOFTWARE\Microsoft\CTF\TIP\{CLSID}\Category\...`（能力类别，至少 GUID_TFCAT_TIP_KEYBOARD）
- **HKCU 注册可用于免 UAC 测试**（per-user COM 优先于 HKLM）。
- 手动写注册表**不如 regsvr32 可靠**——regsvr32 调 DllRegisterServer 走官方 API；但实测手动写齐三处也能被系统识别。
- 诊断工具 `WowTypeDiag.exe` 用官方 API 枚举/验证/激活，是排查"列表里看不到"的权威手段。

## 历次关键 bug 与修复

1. **候选窗 RefCell 重入 panic → 宿主崩溃**：窗口数据改挂 GWLP_USERDATA（无 RefCell）+ 所有 wnd_proc catch_unwind。
2. **WS_EX_LAYERED 窗口 GDI 绘制不可见（Win8+）**：去掉 layered，用普通 TOPMOST popup。
3. **TSF 编辑会话 context thread_local 跨线程失效 → 上屏静默失败**：context 放进 ITfEditSession 结构体。
4. **异步编辑会话 worker 线程 + ITfComposition thread_local → 文本重复/预编辑残留**（代码审查发现）：v13 改为无内联组句方案。
5. **OnTestKeyDown 吃编辑键但 OnKeyDown 返回 FALSE → 键被 TSF 丢弃**（应用收不到退格）：只吃能处理的键。
6. **PowerShell 中文乱码**：Get-Content/Out-File 默认编码破坏 UTF-8 → 一律 ReadAllText/WriteAllText + UTF8Encoding($false)。
7. **cmd 黑窗**：Rust 默认 console subsystem → `#![windows_subsystem = "windows"]`。

## 代码审查报告摘要（v12 → v13 修复依据）

详见 2026-11-15 审查：

- **严重 1**：COMPOSITION thread_local + 异步编辑会话跨线程 → 组句生命周期断裂（v13 用无组句方案彻底规避）。
- **严重 2**：OnTestKeyDown 全吃策略 + OnKeyDown FALSE ≠ 还给应用 → 退格/回车/Esc 被吞（v13 修复）。
- **严重 3**：状态按线程不按 context → 多文档串档（v13 失焦清理缓解，完整修复待做）。
- **中 1**：Session::drop 依赖 TLS 析构顺序（未文档化行为，实测当前工具链静默泄漏）→ try_with + Deactivate 显式释放。
- **中 3**：候选窗类用 EXE hInstance → DLL 卸载后悬空 wnd_proc（用 DLL 自身 hInstance）。
- **中 5**：WM_PAINT 内 panic 跳过 EndPaint → 重绘风暴（EndPaint 保证执行）。
- **中 7**：frame.size 不校验 + cand_count 无上限（钳制 + 校验）。
- **中 8**：CoInitialize S_FALSE 时错误 CoUninitialize。

## 输入法"共存"需求的教训

用户需求演进：
1. 最初要 TSF 系统集成 → 嫌"嵌入系统卡慢"→ 改为独立 App（全局钩子 + SendInput）
2. 独立 App 与搜狗抢键 → 加"检测中文键盘布局自动退让 + Ctrl+Alt+W 三态切换"
3. 最终发现独立 App **不能出现在 Win+Space 列表**、无法像搜狗那样"互相切换" → 回归 TSF

结论：**要做"像搜狗一样能切换的输入法"，TSF 是唯一正路**。内存问题的正解是"薄壳 + 引擎进程"架构，不是回避 TSF。

## 部署/测试注意事项

- 每次换 DLL 版本，**已启动的进程必须重启**才加载新 DLL（TSF 机制，搜狗亦然）。测试新版本：杀记事本 → 开新记事本 → 切换。
- 版本文件命名 `wowtype_tsf_vXX.dll`，注册表指向当前版本，避免 DLL 被占用时无法覆盖。
- 日志：`%LOCALAPPDATA%\WowType\logs\`，`WOWTYPE_DEBUG=1` 才记按键级日志；GBK 控制台看 UTF-8 会乱码但内容正常。
- 内存紧张机器（本机测试时可用内存仅 ~700MB）：中等词库 + 少量进程可用；完整词库 500MB/进程会爆。

## NSIS 要点

- 脚本必须 UTF-8 BOM + `Unicode true`，否则中文乱码。
- `nsExec::ExecToLog 'regsvr32 /s ...'` 注册（静默，UAC 由 installer 的 admin 权限提供）。
- 托盘自启：`HKLM\...\CurrentVersion\Run\WowTypeTray`。
- 卸载：taskkill 托盘 + `regsvr32 /u` + 删文件 + 删注册。

## 后续架构路线（建议）

```
TSF 薄壳 (每进程 ~10MB)
  └─ 命名管道/共享内存 ──► WowTypeService.exe（引擎 + 词库，单实例 230MB）
                             └─ 候选窗进程 + 托盘
```
搜狗/微信输入法均为此架构。TSF 壳需要：按键转发、上屏请求、候选数据回传、焦点/上下文管理。

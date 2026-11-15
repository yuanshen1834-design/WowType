# WowType Windows 版

**WowType 拼音输入法 Windows 移植版**——基于 [Gloridust/WowType](https://github.com/Gloridust/WowType)（MIT OR Apache-2.0 双许可证）的 Rust 拼音引擎，实现可在 Windows 上使用的输入法。

> 本仓库是 WowType 的 Windows 平台移植，核心拼音引擎（`core/`）版权归上游项目及原词库作者所有。本仓库新增代码同样采用 MIT OR Apache-2.0 双许可。

## 完成情况

### ✅ 已完成

| 组件 | 状态 | 说明 |
|------|------|------|
| **TSF 系统输入法** | ✅ 可用 | `windows/wowtype-tsf`，注册后出现在系统输入法列表（Win+Space / Ctrl+Shift 切换） |
| **候选窗口** | ✅ | 拼音行 + 候选行 + 翻页提示，GDI 自绘，TOPMOST 无激活 |
| **生僻字候选** | ✅ | 词库含全部 4.6 万单字，候选上限 100 个，`-` `=` 翻页 |
| **中英文切换** | ✅ | 单按 Shift 切换中英文模式 |
| **数字键选词** | ✅ | 1-9 直接选候选 |
| **托盘程序** | ✅ | `windows/wowtype-tray`，蓝色 W 图标，右键菜单含"切换到 WowType 输入法" |
| **独立版 App** | ✅ 备用 | `windows/wowtype-app`，全局键盘钩子方案（不入系统），带状态条 |
| **诊断工具** | ✅ | `windows/wowtype-diag`，用官方 TSF API 枚举/注册/激活输入法 |
| **NSIS 安装器** | ✅ | `installer/installer.nsi`，经典向导，可选盘符，含注册/托盘自启/卸载 |
| **Windows core 编译** | ✅ | `core` 已在 x86_64-pc-windows-gnu 下编译通过（`llama.rs` 加了 cfg 门控） |

### 词库

- 中等词库（默认）：34.6 万词条 = 30 万高频词 + 全部 4.6 万单字，约 8.5 MB，运行时每进程约 230 MB 内存
- 完整词库：92.9 万词条（rime-ice 生成），约 27 MB，运行时约 500 MB 内存（内存紧张时不推荐）
- 精简词库：9.6 万词条，约 1.9 MB，约 120 MB 内存（候选覆盖不足，已弃用）
- 生成脚本见 `tools/make_medium_lexicon.sh`

### ⚠️ 已知问题 / 遗留

1. **TSF DLL 注入每进程**：每个使用 WowType 的进程独立加载引擎和词库（每进程约 230 MB）。搜狗等商业输入法用"TSF 薄壳 + 独立引擎进程（管道通信）"架构解决，本版本未做，是后续最大的优化方向。
2. **升级需重启应用**：DLL 被进程加载后无法替换，新版需要重启宿主应用（与所有 TSF 输入法一致）。
3. **无词频学习**：未做用户词库/自学习（上游 core 有候选排序能力，但学习持久化未接）。
4. **候选窗无鼠标点选**：目前仅键盘操作（数字/空格选词、翻页）。
5. **非美式键盘布局**：`keys.rs` 的放行键回插字符按美式键盘硬编码。
6. **高分屏 DPI**：候选窗定位未做 Per-Monitor DPI 换算（可能轻微偏移）。
7. **语言栏图标**：TSF DLL 未嵌入图标资源（IconFile 指向 DLL 但无 .ico 资源）。

### ❌ 已放弃方案（存档，勿复用）

- `windows/wowtype-app` 独立版：全局键盘钩子 + SendInput 上屏。**不能出现在系统输入法列表**，与搜狗/微软拼音共存时互相抢键，需要"自动退让"检测才能共存。用户需求"像搜狗一样切换"时放弃。
- TSF v10 及以前版本：候选数 5、内联组句 thread_local（跨线程断裂）、编辑键误吞等 bug，已被 v13 修复。

## 架构

```
┌─────────────────────────────────────────────┐
│  系统输入法列表 (Win+Space / Ctrl+Shift)      │
│  ├─ 搜狗  ├─ 微软拼音  ├─ WowType Pinyin ←─┐ │
└─────────────────────────────────────────────┘
                                            │
        TSF 框架 (ctfmon/TextInputHost)      │
        ┌─────────────────────────────┐     │
        │ wowtype_tsf.dll (每进程注入)  │◄────┘
        │  ├─ COM 文本服务 (ITfKeyEventSink) │
        │  ├─ 候选窗口 (GDI 自绘)            │
        │  └─ 引擎 (wowtype_core.dll + 词库) │
        └─────────────────────────────┘
        WowTypeTray.exe（托盘，独立进程，自启动）
        WowTypeDiag.exe（诊断/注册工具）
```

### 数据流

按键 → `OnTestKeyDown`（只吃能处理的键）→ `OnKeyDown` → 引擎会话 `session_key` → `FrameSnapshot`（commit/preedit/cand_count）→ 上屏 `InsertTextAtSelection`（无内联组句，拼音显示在候选窗）→ 候选窗刷新。

### v13 关键设计决策（来自代码审查）

1. **无内联组句**：TSF 异步编辑会话在 worker 线程执行，`ITfComposition` 若存 thread_local 会跨线程断裂（文本重复/预编辑残留）。改为拼音显示在候选窗、上屏直接 `InsertTextAtSelection`，彻底消除跨线程状态。
2. **OnTestKeyDown 保守吃键**：TSF 语义下 `OnTestKeyDown` 返回 TRUE 的键**不会再传给应用**。因此只吃"引擎会处理"或"能无损回插"的键；编辑键（退格/回车/Esc/方向键）仅在拼音编辑中才拦截。
3. **所有 FFI 入口 catch_unwind**：TSF 从 C++ 调进 Rust，panic 越过 FFI 边界 = 宿主进程 abort。所有高危入口（按键/编辑会话/窗口过程）已防护。
4. **ABI 校验**：`WowtypeFrame.size` 校验防版本错位读垃圾值；`cand_count` 钳制防 UI 冻结。
5. **窗口类用 DLL 自身 hInstance**：防 DLL 卸载后悬空 wnd_proc。

## 目录结构

```
├── windows/
│   ├── wowtype-tsf/     # TSF 系统输入法（主方案，v13 稳定版）
│   ├── wowtype-app/     # 独立版 App（全局钩子，备用/已弃）
│   ├── wowtype-tray/    # 托盘程序（蓝色 W 图标）
│   └── wowtype-diag/    # 诊断/注册工具（TSF API）
├── tools/               # 构建脚本（WSL 交叉编译）
├── installer/           # NSIS 安装脚本
├── docs/                # 开发笔记
└── README.md
```

## 构建

### 环境要求

- **必须在 WSL (Ubuntu) 交叉编译**：Windows 侧 GNU 工具链的 dlltool 损坏，MSVC 未装
- WSL 依赖：`mingw-w64`（linker）、Rust stable（target `x86_64-pc-windows-gnu`）
- Rust 工具链安装（镜像不稳时用官方源）：
  ```bash
  RUSTUP_DIST_SERVER=https://static.rust-lang.org rustup default stable
  rustup target add x86_64-pc-windows-gnu
  ```
- cargo 镜像：rsproxy.cn sparse 源（见各 crate 的 .cargo/config.toml）

### 编译步骤

```bash
# core 引擎（上游源码，在 src/WowType-main）
cargo build --release --target x86_64-pc-windows-gnu   # 产出 wowtype_core.dll

# TSF 输入法
bash tools/wsl_build_tsf.sh      # 产出 wowtype_tsf.dll
# 托盘
bash tools/wsl_build_tray.sh     # 产出 WowTypeTray.exe
# 诊断工具
bash tools/wsl_build_diag.sh     # 产出 WowTypeDiag.exe

# 安装包（Windows 侧）
tools/nsis/nsis-3.11/makensis.exe installer/installer.nsi   # WowTypeSetup.exe
```

### 数据文件

引擎在 DLL 同目录 `data/` 下找词库：
- `lexicon.tsv`（词条，优先 `lexicon_small.tsv` 若存在）
- `words.lm`（语言模型）
- `english.txt`（英文词库）

## 使用

1. 运行安装包（可选任意盘符），自动注册 + 托盘自启
2. `Win+Space` 或 `Ctrl+Shift` 切到「WowType Pinyin」
3. 打拼音 → 候选窗（拼音在上、候选在下）→ 数字键/空格选词，`-` `=` 翻页
4. 单按 Shift 切换中英文
5. 托盘蓝色 W 图标：右键 →「切换到 WowType 输入法」/ 打开数据目录 / 退出

### 手动注册（免安装）

```powershell
regsvr32 wowtype_tsf.dll                # 注册（管理员）
WowTypeDiag.exe                         # 验证/修复/激活
regsvr32 /u wowtype_tsf.dll             # 卸载
```

## 环境变量

- `WOWTYPE_DEBUG=1`：详细按键日志
- `WOWTYPE_NO_LM=1`：不加载语言模型（省内存，长句质量下降）
- 日志位置：`%LOCALAPPDATA%\WowType\logs\`

## 后续开发建议（给接续者）

按优先级排序：

1. **TSF 薄壳 + 引擎进程架构**：把引擎/词库挪到独立进程（管道/共享内存通信），DLL 只做转发。内存从"每进程 230 MB"降到"引擎进程一份 230 MB + 每进程壳约 10 MB"。这是与搜狗等商业输入法对齐的关键一步。
2. **用户词库/自学习**：候选排序目前全依赖静态词库。
3. **候选窗鼠标点选**：WM_LBUTTONUP 命中检测 + 上屏。
4. **语言栏/托盘图标资源**：给 TSF DLL 嵌入 .ico（windres 编译 .rc），设置 IconFile/IconIndex。
5. **TSF Composition 内联显示**：如果做回内联拼音，务必用 `Arc<Mutex<Option<ITfComposition>>>` 随编辑会话传递（见 docs/development-notes.md 的审查报告）。
6. **多 context 状态隔离**：当前状态按线程，多文档应用（Word 开两个文档）会串档；应按 `ITfContext` 键控。

## 致谢

- [Gloridust/WowType](https://github.com/Gloridust/WowType)：核心拼音引擎（MIT OR Apache-2.0）
- [rime-ice](https://github.com/iDvel/rime-ice)：词库来源
- 青简输入法（GPL-3.0）：TSF 架构参考（仅学习架构，未复制代码）

## 许可证

MIT OR Apache-2.0（与上游一致）

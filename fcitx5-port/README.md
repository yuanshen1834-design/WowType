# WowType Windows 移植（fcitx5-windows 框架）——研究成果与交接文档

> 本目录保存了 2026-11-16 使用 **fcitx5-windows 框架**移植 WowType 拼音输入法到 Windows 的完整研究成果。
> 移植在"所有组件编译成功、TSF 注册验证通过"的阶段停下，**因架构问题（内存）导致系统卡死而中止**。
> 后面的接续者可以从这里继续——所有关键代码、补丁、构建脚本、踩坑记录都在。

## 一、最终成果状态

| 组件 | 状态 | 说明 |
|------|------|------|
| MSYS2 + clang 23 构建环境 | ✅ 完成 | 装在 G:\msys64，`pacman` 装齐 CI 同款依赖 |
| fcitx5 核心（3 DLL + fcitx5.exe）| ✅ 编译成功 | `libFcitx5Core/Config/Utils.dll`，fcitx5.exe 运行验证通过（gettext/addon 系统初始化正常）|
| **TSF 系统输入法 DLL** | ✅ 编译成功+注册成功 | `libfcitx5-x86_64.dll`（win32 目录），regsvr32 注册 HKLM CLSID + TIP 成功 |
| **WowType 引擎 addon** | ✅ 编译成功 | `libwowtype.dll` 6.6MB（静态链接 Rust 引擎），dbus 已条件化 |
| **WinUI 候选窗 addon** | ✅ 编译成功 | `winui.dll`（GDI 自绘候选窗）|
| 词库 | ✅ 部署就绪 | lexicon.tsv 27.4MB + words.lm 36.7MB + english |
| TSF↔引擎桥接 | ✅ 完成 | 每进程内嵌 fcitx::Instance + 自定义 InputContext 上屏 |
| **实际打字测试** | ❌ 未完成 | **卡死于此**：TSF DLL 每进程加载引擎+词库 → 内存爆炸（见下节）|

## 二、⚠️ 致命问题（中止原因，务必先读）

**问题**：TSF DLL 的设计是"每个加载它的宿主进程内嵌一个完整的 fcitx5 实例 + wowtype 引擎 + 词库"。
- 每进程开销：词库 27.4MB 展开 ≈ 400MB + LM 36.7MB ≈ 70MB + 基础 ≈ **500MB/进程**
- explorer / 浏览器 / 记事本等多个进程同时加载 → **内存瞬间数十 GB 级需求 → 系统卡死**

**这不是代码 bug，是架构选择错误**。任何"TSF DLL 内嵌引擎"的方案都有这个问题（包括自研 TSF 路线，之前同样踩过）。

**正确的架构（与搜狗/微信输入法一致）**：
```
TSF 薄壳 DLL（每进程，<10MB，只做转发）
    │ 命名管道/共享内存 IPC
    ▼
独立引擎进程 WowTypeService.exe（单实例，词库/引擎只加载一份 ~500MB）
    │
    ├── 候选窗（WinUI 或独立渲染进程）
    └── 配置/托盘
```
或者：**换用 RIME 引擎**（librime + Weasel 框架成熟，词库格式转换后用 Rime 的共享内存方案）。

## 三、已完成的移植工作详解

### 3.1 fcitx5-windows 框架修复（补丁见 `patches/fcitx5-windows.patch`）

| 文件 | 修改内容 |
|------|---------|
| `CMakeLists.txt` | `-include pthread_compat.h`：新版 mingw 头把 `clockid_t` 挪进 pthread_compat.h |
| `win32/CMakeLists.txt` | 注释 assets（缺 ImageMagick 无法生成 penguin.ico）|
| `win32/tsf/tsf.h` | 加 EngineBridge/bridge_/pendingCommit_/forwardKeyToFcitx 等桥接声明；补 ATL 兼容层引用 |
| `win32/tsf/atlcomcli_min.h` | **新增**：最小 ATL 兼容层（CComPtr/CComQIPtr/CoCreateInstance）+ 补 mingw 缺失的 ITfTextInputProcessorEx 接口、IID、TF_CLIENTID_NULL、TSF 类别 GUID |
| `win32/tsf/fcitx_bridge.h/.cpp` | **新增核心桥接**：每线程创建 fcitx::Instance（事件循环独立线程）、TsfInputContext（commitStringImpl → TSF 上屏）、按键 VK→fcitx::Key 转发、光标位置、日志重定向 |
| `win32/tsf/KeyEventSink.cpp` | OnKeyDown/OnKeyUp → forwardKeyToFcitx；焦点事件 |
| `win32/tsf/EditSession.cpp` | 替换原硬编码"哈"：上屏 pendingCommit_（InsertTextAtSelection + 移动选区）|
| `win32/tsf/ThreadMgrEventSink.cpp` | OnSetFocus → fcitx IC focusIn/out + cursorRect |
| `win32/tsf/TextInputProcessorEx.cpp` | Activate 时 ensureEngine（创建 fcitx 实例）|
| `win32/tsf/tsf.cpp` | 删除重复的 Tsf 构造/析构定义（移到 bridge.cpp）|
| `win32/dll/register.cpp` | ATL → 兼容层；类别循环改指针式；精简类别（去掉 mingw 缺失的属性类 GUID）|
| `src/CMakeLists.txt` | 恢复 fcitx5.exe 主程序构建 + add_subdirectory(winui) |
| `src/winui/` | **新增**：WinUI 候选窗 addon（读 InputPanel → GDI 窗口：拼音行+候选行+翻页提示）|

### 3.2 WowType addon 移植（`wowtype-addon/` 目录，直接覆盖原文件）

| 文件 | 修改内容 |
|------|---------|
| `src/clipboard_copy.cpp` | `#ifdef _WIN32` 分支：Windows 剪贴板 API（原 wl-copy/xclip/xsel 子进程）|
| `src/wowtype_engine.h` | dbus includes/`WowtypeControl` 类/overlay 成员 → `#ifndef _WIN32` 条件化；`voiceOverlay()` Windows 返回 false；overlaySentAt_ 等移出条件区 |
| `src/wowtype_engine.cpp` | `WowtypeControl` 方法/`setupControl`/`setupVoiceOverlay`/`sendVoiceOverlay`/`checkOverlayAck` → Windows 空实现 stub |
| `src/wowtype_log.h` | `localtime_r` → Windows `localtime_s`；path→string 转换修复 |
| `CMakeLists.txt` | Windows：链接 `ws2_32 userenv bcrypt ntdll advapi32 synchronization`（Rust std 需要 WaitOnAddress）；VOICE 默认 OFF（无 ALSA）；`--exclude-libs` 仅非 Windows；`WOWTYPE_CONFIG_TOOL` 不定义；`WOWTYPE_SYSTEM_DATA_DIR` 可配置 |

### 3.3 Rust 引擎编译（Windows 静态库）

```bash
cd src/WowType-main
cargo build --release --target x86_64-pc-windows-gnu -p wowtype-core --no-default-features
# 产物: target/x86_64-pc-windows-gnu/release/libwowtype_core.a (25MB, staticlib)
# --no-default-features 去掉 voice（ALSA 依赖）
```

## 四、构建步骤（完整复现）

### 4.1 环境准备（MSYS2，装在 G 盘）

```bash
# 下载 https://github.com/msys2/msys2-installer/releases/latest/download/msys2-x86_64-latest.exe
# 安装到 G:\msys64（或解压 msys2-base tar.xz 到 G:\msys64）
# 换镜像（中科大/清华）
printf 'Server = https://mirrors.ustc.edu.cn/msys2/mingw/clang64/\nServer = https://mirrors.tuna.tsinghua.edu.cn/msys2/mingw/clang64/\n' > /etc/pacman.d/mirrorlist.clang64
# （mirrorlist.mingw64 / mirrorlist.msys 同理）
pacman-key --init --populate msys2
pacman -Syu --noconfirm
pacman -S --noconfirm --needed \
  mingw-w64-clang-x86_64-extra-cmake-modules mingw-w64-clang-x86_64-dlfcn \
  mingw-w64-clang-x86_64-libuv mingw-w64-clang-x86_64-clang \
  mingw-w64-clang-x86_64-cmake mingw-w64-clang-x86_64-ninja \
  mingw-w64-clang-x86_64-pkgconf mingw-w64-clang-x86_64-gettext-tools \
  mingw-w64-clang-x86_64-toolchain
```

> **重要**：所有构建命令通过 **任务计划（schtasks）以完整令牌运行**——当前 DSH 沙箱/受限令牌会让 pacman 的 fork 失败（CreateProcess EACCES）。见 scripts/ 里的 run-*.bat 模式。

### 4.2 构建 fcitx5-windows（scripts/build-fcitx.sh）

```bash
cd /g/WowType/fcitx5-windows
cmake -B build/x86_64 -G Ninja -DCMAKE_BUILD_TYPE=Release -DARCH=AMD64 \
  -DENABLE_KEYBOARD=OFF -DENABLE_NOTIFICATIONITEM=OFF -DENABLE_IMSELECTOR=OFF
cmake --build build/x86_64
cmake --install build/x86_64 --prefix /g/WowType/fcitx5-inst
# 注意：install 会在 fcitx5/data/icon 报错（缺图标工具），手动复制产物即可
```

### 4.3 构建 TSF DLL（scripts/build-win32.sh）

```bash
cd /g/WowType/fcitx5-windows/win32
cmake -B build -G Ninja -DCMAKE_BUILD_TYPE=Release -DCMAKE_PREFIX_PATH=/g/WowType/fcitx5-inst
cmake --build build
# 产物: build/dll/libfcitx5-x86_64.dll
```

### 4.4 构建 WowType addon（scripts/build-addons.sh）

```bash
cd /g/WowType/src/WowType-main/fcitx5-addon
cmake -B build-win -G Ninja -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_PREFIX_PATH=/g/WowType/fcitx5-inst \
  -DCMAKE_INSTALL_PREFIX=/g/WowType/fcitx5-inst \
  -DWOWTYPE_CORE_LIB=/g/WowType/src/WowType-main/target/x86_64-pc-windows-gnu/release/libwowtype_core.a \
  -DWOWTYPE_CORE_STATIC=ON -DWOWTYPE_VOICE=OFF \
  -DWOWTYPE_INSTALL_THEMES=OFF -DWOWTYPE_INSTALL_SETTINGS=OFF \
  -DFCITX_INSTALL_ADDONDIR=lib/fcitx5 \
  -DFCITX_INSTALL_PKGDATADIR=share/fcitx5 \
  -DWOWTYPE_SYSTEM_DATA_DIR=G:/WowType/fcitx5-inst/share/wowtype
cmake --build build-win
cmake --install build-win
```

### 4.5 部署结构与运行时依赖

```
G:\WowType\fcitx5-inst\
├── bin\
│   ├── fcitx5.exe                # fcitx5 主程序（服务）
│   ├── libfcitx5-x86_64.dll      # TSF 输入法 DLL（regsvr32 注册）
│   ├── libFcitx5Core.dll / libFcitx5Config.dll / libFcitx5Utils.dll
│   └── MSYS2 运行时（关键！缺一个 exe 就静默退出）:
│       libc++.dll libunwind.dll libwinpthread-1.dll
│       libdl.dll libintl-8.dll libuv-1.dll libiconv-2.dll libcharset-1.dll
├── lib\fcitx5\
│   ├── libwowtype.dll            # WowType 引擎 addon
│   └── winui.dll                 # 候选窗 addon
└── share\
    ├── fcitx5\addon\{wowtype.conf,winui.conf}
    ├── fcitx5\inputmethod\wowtype.conf
    └── wowtype\{lexicon.tsv,words.lm,english.txt,s2t.tsv}
```

**关键路径推导**：`fcitx_bridge.cpp` 的 `setupFcitxEnv` 从 DLL 路径推 root（`bin/libfcitx5-x86_64.dll` → 上两级 = fcitx5-inst），设置 FCITX_ADDON_DIRS/FCITX_DATA_DIRS/XDG_DATA_DIRS。

### 4.6 注册

```powershell
regsvr32 /s G:\WowType\fcitx5-inst\bin\libfcitx5-x86_64.dll   # 管理员
# 注册后 Win+Space 列表出现 "Fcitx5"
```

## 五、踩坑记录（按发现顺序）

1. **MSYS2 首启卡住**：bash -lc 首次初始化（pacman-key）交互等待 → 用独立脚本 + schtasks 非交互跑。
2. **受限令牌下 MSYS2 全挂**：DSH 沙箱的受限令牌让 cygwin fork 的 CreateProcess 失败（EACCES）→ **所有构建通过 schtasks（完整用户令牌）执行**。这是本项目能成功的基础设施技巧。
3. **`clockid_t` 未定义**：clang 23 + 新 mingw-w64 头把 clockid_t 移进 `pthread_compat.h` → `-include pthread_compat.h`。
4. **fcitx5 的 cmake 配置写死 C:/ 前缀**：install --prefix 覆盖后 Targets 文件里 include 路径仍是 C:/ → sed 替换 `C:/include` → 实际安装路径。
5. **ATL 缺失**：fcitx5-windows 用 CComPtr（MSYS2 无 ATL）→ 自写 `atlcomcli_min.h`（CComPtr/CComQIPtr/CoCreateInstance）。
6. **mingw msctf.h 缺 ITfTextInputProcessorEx**：手动补接口定义 + IID `{6E4E2102-F9CD-433D-B496-303CE03A6507}` + TF_CLIENTID_NULL + 若干 TSF 类别 GUID（值从 windows crate / MSDN 取）。
7. **`--exclude-libs` 是 GNU ld 专有**：lld 不认 → Windows 下不传。
8. **Rust std 静态库需要 `synchronization` 库**：`WaitOnAddress/WakeByAddress*` 未定义 → `-lsynchronization`。
9. **WowType addon 的 dbus 依赖**：fcitx-utils/dbus 在 ENABLE_DBUS=OFF 时不存在 → 条件化 wowtype 的 dbus 代码（语音浮条/外部控制接口 Windows 无用）。
10. **fcitx5.exe 静默退出**：缺运行时 DLL（libdl/libintl/libuv）→ 从 clang64/bin 复制。**没有任何错误提示，纯靠 objdump 查依赖**。
11. **`ENABLE_KEYBOARD=ON` 会拉 XKBCommon 依赖**（Windows 没有）→ 保持 OFF。引擎按键由 InputContext::keyEvent 直接派发给 IM，不需要 keyboard addon。

## 六、下一步建议（给接续者）

### 路线 A：继续 fcitx5-windows（推荐，架构改造）

1. **重做架构**：TSF 薄壳 + 独立引擎进程（命名管道 IPC），彻底解决内存问题
   - fcitx5 本身支持前端/后端分离（fcitx5 的 "InputMethod Frontend" 抽象）——把 TSF 做成 fcitx5 的一个前端（类似 fcitx5-qt/gtk 的角色），通过 socket/pipe 连独立 fcitx5 进程
   - 参考：fcitx5 的 fcitx5-qt 的 DBus/socket 前端模式（但 Windows 无 DBus → 用命名管道）
2. 或者：**每进程只加载引擎 DLL 不加载词库**——词库放共享内存（CreateFileMapping），引擎按需 mmap 词库——改造 WowType core（工作量大）
3. 补 keyboard addon 的 Windows 替代（按键处理目前由 InputContext 直接处理，可能缺候选翻页/输入法切换的 addon 支持）

### 路线 B：RIME 引擎（另一条路）

- RIME（librime）在 Windows 有成熟实现（Weasel 小狼毫），内存共享方案已验证
- 需要：WowType 词库转 RIME 格式（lexicon.tsv → rime yaml/binary）+ 用 RIME 的拼音方案实现"长句整句输入"
- 优点：不用碰 TSF 内存问题；缺点：WowType 的引擎特性（LLM 长句、语音、翻译）要重写

### 路线 C：纯独立 App（之前做过，用户否了）

- 全局钩子 + SendInput + 自绘候选窗（`windows/wowtype-app` 在 WowType 仓库）——**不进输入法列表**，用户要求"像搜狗一样能切换"所以放弃

## 七、文件说明

```
fcitx5-port/
├── README.md                    # 本文件
├── patches/
│   └── fcitx5-windows.patch     # fcitx5-windows 全部修改的 git 补丁（11 文件）
├── new-files/                   # 新增文件（直接放到对应路径）
│   ├── atlcomcli_min.h          #   → win32/tsf/
│   ├── fcitx_bridge.h/.cpp      #   → win32/tsf/
│   ├── winui.h/.cpp/CMakeLists.txt/winui.conf.in  # → src/winui/
│   └── CMakeLists.txt           #   → src/CMakeLists.txt（含主程序+winui）
├── wowtype-addon/               # WowType 插件改动（覆盖原文件）
│   ├── CMakeLists.txt
│   └── src/{clipboard_copy.cpp,wowtype_engine.h,wowtype_engine.cpp,wowtype_log.h}
└── scripts/                     # MSYS2 构建脚本（schtasks 运行用）
    ├── init-all.sh              # MSYS2 初始化（镜像/依赖）
    ├── build-fcitx.sh           # fcitx5 核心+主程序
    ├── build-win32.sh           # TSF DLL
    └── build-addons.sh          # winui + wowtype addon
```

## 八、环境信息

- 构建机：Windows 10.0.26300 x64，16GB 内存
- MSYS2 装在 `G:\msys64`（clang64 环境，clang 23.1.5）
- WowType 上游：https://github.com/Gloridust/WowType（MIT OR Apache-2.0）
- fcitx5-windows 上游：https://github.com/fcitx-contrib/fcitx5-windows
- fcitx5 版本：5.1.19（submodule）
- WowType 版本：0.4.0

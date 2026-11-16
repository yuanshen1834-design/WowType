/*
 * WowType Windows 移植：TSF ↔ fcitx5 引擎桥实现。
 * 结构定义在 fcitx_bridge.h；这里放引擎生命周期、按键转发、上屏。
 * SPDX-License-Identifier: LGPL-2.1-or-later
 */
#include "tsf.h"
#include "fcitx_bridge.h"
#include "../dll/register.h"

#include <fcitx/addonmanager.h>
#include <fcitx/event.h>
#include <fcitx/inputcontext.h>
#include <fcitx/inputcontextmanager.h>
#include <fcitx/instance.h>
#include <fcitx-utils/eventdispatcher.h>
#include <fcitx-utils/event.h>
#include <fcitx-utils/keysym.h>
#include <fcitx-utils/keysymgen.h>

#include <filesystem>

// dll/main.cpp 的引用计数（TSF DLL 生命周期）
extern void DllAddRef();
extern void DllRelease();

namespace fcitx {

namespace {

// fcitx5 的数据/插件目录：DLL 的上级目录结构（与主程序 main.cpp 的 setupEnv 一致）
void setupFcitxEnv(HMODULE dllModule) {
    wchar_t path[MAX_PATH]{};
    GetModuleFileNameW(dllModule, path, MAX_PATH);
    std::filesystem::path dllPath(path);
    auto rootPath = dllPath.parent_path().parent_path();
    if (!GetEnvironmentVariableW(L"FCITX_ADDON_DIRS", nullptr, 0)) {
        std::wstring addonDirs = (rootPath / L"lib" / L"fcitx5").wstring();
        SetEnvironmentVariableW(L"FCITX_ADDON_DIRS", addonDirs.c_str());
    }
    if (!GetEnvironmentVariableW(L"XDG_DATA_DIRS", nullptr, 0)) {
        std::wstring dataDirs = (rootPath / L"share").wstring();
        SetEnvironmentVariableW(L"XDG_DATA_DIRS", dataDirs.c_str());
    }
    if (!GetEnvironmentVariableW(L"FCITX_DATA_DIRS", nullptr, 0)) {
        std::wstring fcitxData = (rootPath / L"share" / L"fcitx5").wstring();
        SetEnvironmentVariableW(L"FCITX_DATA_DIRS", fcitxData.c_str());
    }
}

// 从 VK + 修饰键构造 fcitx::Key。
Key keyFromVk(UINT vk, KeyStates states) {
    KeySym sym = FcitxKey_None;
    UINT scan = MapVirtualKeyW(vk, MAPVK_VK_TO_VSC);
    wchar_t buf[8]{};
    BYTE kbd[256]{};
    if (ToUnicode(vk, scan, kbd, buf, 8, 0) > 0) {
        for (wchar_t *p = buf; *p; ++p) {
            KeySym s = Key::keySymFromUnicode(*p);
            if (s != FcitxKey_None) {
                sym = s;
                break;
            }
        }
    }
    if (sym == FcitxKey_None) {
        switch (vk) {
        case VK_BACK: sym = FcitxKey_BackSpace; break;
        case VK_RETURN: sym = FcitxKey_Return; break;
        case VK_ESCAPE: sym = FcitxKey_Escape; break;
        case VK_TAB: sym = FcitxKey_Tab; break;
        case VK_SPACE: sym = FcitxKey_space; break;
        case VK_LEFT: sym = FcitxKey_Left; break;
        case VK_RIGHT: sym = FcitxKey_Right; break;
        case VK_UP: sym = FcitxKey_Up; break;
        case VK_DOWN: sym = FcitxKey_Down; break;
        case VK_HOME: sym = FcitxKey_Home; break;
        case VK_END: sym = FcitxKey_End; break;
        case VK_PRIOR: sym = FcitxKey_Page_Up; break;
        case VK_NEXT: sym = FcitxKey_Page_Down; break;
        case VK_DELETE: sym = FcitxKey_Delete; break;
        case VK_APPS: sym = FcitxKey_Menu; break;
        case VK_LSHIFT: sym = FcitxKey_Shift_L; break;
        case VK_RSHIFT: sym = FcitxKey_Shift_R; break;
        case VK_LCONTROL: sym = FcitxKey_Control_L; break;
        case VK_RCONTROL: sym = FcitxKey_Control_R; break;
        case VK_LMENU: sym = FcitxKey_Alt_L; break;
        case VK_RMENU: sym = FcitxKey_Alt_R; break;
        default:
            if (vk >= '0' && vk <= '9') sym = static_cast<KeySym>(vk);
            else if (vk >= 'A' && vk <= 'Z') sym = static_cast<KeySym>(vk + 32);
            break;
        }
    }
    return Key(sym, states);
}

} // namespace

// ---- TsfInputContext 实现 ----
TsfInputContext::TsfInputContext(InputContextManager &manager, Tsf *owner)
    : InputContext(manager, "windows"), owner_(owner) {}

void TsfInputContext::commitStringImpl(const std::string &text) {
    owner_->onFcitxCommit(text);
}

// ---- EngineBridge 实现 ----
void Tsf::EngineBridge::start(Tsf *self) {
    owner = self;
    // 诊断日志：宿主进程无 stderr，把 fcitx 日志重定向到文件
    {
        static FILE *logFile = nullptr;
        if (!logFile) {
            logFile = fopen("G:/WowType/fcitx5-inst/tsf-engine.log", "a");
            if (logFile) {
                fclose(stderr);
                *stderr = *logFile;
                setvbuf(stderr, nullptr, _IONBF, 0);
            }
        }
    }
    fcitx::Log::setLogRule("*=5,notimedate");
    instance = std::make_unique<Instance>(0, nullptr);
    auto &mgr = instance->addonManager();
    mgr.registerDefaultLoader(nullptr);
    instance->initialize();
    dispatcher = std::make_unique<EventDispatcher>();
    dispatcher->attach(&instance->eventLoop());
    ic = new TsfInputContext(instance->inputContextManager(), owner);
    ic->focusIn();
    ready.store(true);
    instance->eventLoop().exec();
    ready.store(false);
    ic->focusOut();
    delete ic;
    ic = nullptr;
    dispatcher.reset();
    instance.reset();
}

void Tsf::EngineBridge::stop() {
    if (!instance) {
        return;
    }
    instance->eventLoop().exit();
    if (eventLoopThread.joinable()) {
        eventLoopThread.join();
    }
}

// ---- Tsf 方法 ----
Tsf::Tsf() { DllAddRef(); }

Tsf::~Tsf() {
    destroyEngine();
    DllRelease();
}

void Tsf::ensureEngine() {
    if (bridge_ && bridge_->ready.load()) {
        return;
    }
    destroyEngine();
    setupFcitxEnv(fcitx::dllInstance);
    bridge_ = std::make_unique<EngineBridge>();
    bridge_->startAsync();
}

void Tsf::destroyEngine() {
    if (!bridge_) {
        return;
    }
    bridge_->stop();
    bridge_.reset();
}

bool Tsf::engineReady() const { return bridge_ && bridge_->ready.load(); }

void Tsf::onFcitxCommit(const std::string &utf8) {
    int len = MultiByteToWideChar(CP_UTF8, 0, utf8.data(), (int)utf8.size(),
                                  nullptr, 0);
    if (len <= 0) {
        return;
    }
    std::wstring wide(len, L'\0');
    MultiByteToWideChar(CP_UTF8, 0, utf8.data(), (int)utf8.size(), wide.data(),
                        len);
    {
        std::lock_guard<std::mutex> lk(commitMutex_);
        pendingCommit_ += wide;
    }
    if (textEditSinkContext_) {
        HRESULT hrSession;
        textEditSinkContext_->RequestEditSession(clientId_, this,
                                                 TF_ES_ASYNCDONTCARE |
                                                     TF_ES_READWRITE,
                                                 &hrSession);
    }
}

bool Tsf::forwardKeyToFcitx(UINT vk, bool isRelease) {
    if (!engineReady() || !bridge_ || !bridge_->ic) {
        return false;
    }
    KeyStates mods = KeyState::NoState;
    if (GetKeyState(VK_SHIFT) & 0x8000) mods |= KeyState::Shift;
    if (GetKeyState(VK_CONTROL) & 0x8000) mods |= KeyState::Ctrl;
    if (GetKeyState(VK_MENU) & 0x8000) mods |= KeyState::Alt;
    if ((GetKeyState(VK_LWIN) | GetKeyState(VK_RWIN)) & 0x8000)
        mods |= KeyState::Super;
    auto key = keyFromVk(vk, mods);

    bool handled = false;
    bridge_->runOnLoop([&]() {
        KeyEvent event(bridge_->ic, key, isRelease);
        handled = bridge_->ic->keyEvent(event);
    });
    return handled;
}

void Tsf::updateCursorRect(ITfContext *pContext) {
    if (!engineReady() || !bridge_ || !bridge_->ic) {
        return;
    }
    GUITHREADINFO info{};
    info.cbSize = sizeof(info);
    if (!GetGUIThreadInfo(0, &info) || info.hwndCaret == nullptr) {
        return;
    }
    RECT rc = info.rcCaret;
    POINT pt{rc.left, rc.bottom + 2};
    if (!ClientToScreen(info.hwndCaret, &pt)) {
        return;
    }
    fcitx::Rect rect(pt.x, pt.y, rc.right - rc.left, rc.bottom - rc.top);
    bridge_->runOnLoop([&]() { bridge_->ic->setCursorRect(rect); });
}

} // namespace fcitx

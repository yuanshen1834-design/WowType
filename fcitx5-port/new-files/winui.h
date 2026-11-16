/*
 * WowType Windows 移植：WinUI addon —— fcitx5 的 Windows 候选窗显示层。
 *
 * fcitx5 在 Windows 上没有任何 UI addon（classicui 依赖 X11、kimpanel 依赖 DBus），
 * 这个 addon 补上最小的一块：读 InputPanel（拼音 + 候选 + 翻页），
 * 用 Win32 GDI 画一个无激活的置顶候选窗。
 *
 * SPDX-License-Identifier: LGPL-2.1-or-later
 */
#ifndef _WINUI_H_
#define _WINUI_H_

#include <fcitx/addoninstance.h>
#include <fcitx/userinterface.h>

namespace fcitx {

class Instance;

class WinUi final : public UserInterface {
public:
    WinUi(Instance *instance);
    ~WinUi() override;

    Instance *instance() const { return instance_; }

    // UserInterface
    void update(UserInterfaceComponent component,
                InputContext *inputContext) override;
    bool available() override;
    void suspend() override;
    void resume() override;

    // 窗口管理（TSF 层也可直接调用 show/hide）
    void showPanel(InputContext *ic);
    void hidePanel();

private:
    Instance *instance_;
    void *win_; // WinWindow 实现细节（避免在头文件暴露 windows.h）
    bool suspended_ = false;
};

} // namespace fcitx

#endif // _WINUI_H_

/*
 * WowType Windows 移植：WinUI addon 实现。
 * 候选窗：WS_POPUP + TOPMOST 无激活窗口，GDI 自绘。
 * 数据来源：fcitx::InputPanel（引擎/TSF 写入），update() 时重绘。
 */
#include "winui.h"

#include <windows.h>
#include <windowsx.h>

#include <algorithm>
#include <string>
#include <vector>

#include <fcitx/candidatelist.h>
#include <fcitx/inputpanel.h>
#include <fcitx/instance.h>
#include <fcitx/statusarea.h>
#include <fcitx/text.h>
#include <fcitx/userinterfacemanager.h>
#include <fcitx-utils/i18n.h>
#include <fcitx/addonfactory.h>
#include <fcitx/addonmanager.h>

namespace fcitx {

namespace {

constexpr const wchar_t *kWindowClass = L"WowTypeWinUiCandidate";
constexpr int kFontSize = 20;
constexpr int kPreeditFontSize = 16;
constexpr int kPaddingX = 10;
constexpr int kPaddingY = 6;
constexpr int kItemGap = 8;
constexpr int kItemHeight = 26;
constexpr int kPreeditHeight = 24;

struct PanelSnapshot {
    std::string preedit;
    std::vector<std::string> candidates;
    int cursor = -1;
    int pageSize = 0;
    bool horizontal = true;
    bool hasPrev = false;
    bool hasNext = false;
    bool empty() const { return preedit.empty() && candidates.empty(); }
};

std::wstring toWide(const std::string &s) {
    if (s.empty()) {
        return {};
    }
    int len = MultiByteToWideChar(CP_UTF8, 0, s.data(), (int)s.size(), nullptr, 0);
    std::wstring out(len, L'\0');
    MultiByteToWideChar(CP_UTF8, 0, s.data(), (int)s.size(), out.data(), len);
    return out;
}

// 收集面板数据（在 update() 里调用，把所有要画的东西拷成快照）
PanelSnapshot snapshotPanel(InputContext *ic) {
    PanelSnapshot snap;
    auto &panel = ic->inputPanel();
    snap.preedit = panel.preedit().toString();
    if (auto list = panel.candidateList()) {
        snap.cursor = list->cursorIndex();
        snap.pageSize = list->size();
        snap.horizontal = list->layoutHint() == CandidateLayoutHint::Horizontal;
        const int n = list->size();
        for (int i = 0; i < n; ++i) {
            snap.candidates.push_back(list->candidate(i).text().toString());
        }
        if (auto *pageable = dynamic_cast<PageableCandidateList *>(list.get())) {
            snap.hasPrev = pageable->hasPrev();
            snap.hasNext = pageable->hasNext();
        }
    }
    return snap;
}

// GDI 文本宽度
int textWidth(HDC hdc, const std::wstring &text, int fontSize, HFONT font) {
    HGDIOBJ old = SelectObject(hdc, font);
    SIZE size{};
    GetTextExtentPoint32W(hdc, text.data(), (int)text.size(), &size);
    SelectObject(hdc, old);
    return size.cx;
}

struct WinWindow {
    HWND hwnd = nullptr;
    PanelSnapshot snap;
    HFONT font = nullptr;
    HFONT preeditFont = nullptr;
    HFONT hintFont = nullptr;
    POINT pos{0, 0};

    ~WinWindow() {
        if (hwnd) {
            DestroyWindow(hwnd);
        }
        if (font) DeleteObject(font);
        if (preeditFont) DeleteObject(preeditFont);
        if (hintFont) DeleteObject(hintFont);
    }

    void create() {
        HINSTANCE hinst = GetModuleHandleW(nullptr);
        WNDCLASSW wc{};
        wc.lpfnWndProc = WinWindow::wndProc;
        wc.hInstance = hinst;
        wc.lpszClassName = kWindowClass;
        wc.hCursor = LoadCursorW(nullptr, reinterpret_cast<LPCWSTR>(IDC_ARROW));
        RegisterClassW(&wc);
        hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW, kWindowClass,
            L"", WS_POPUP, 0, 0, 10, 10, nullptr, nullptr, hinst, this);
        font = CreateFontW(-kFontSize, 0, 0, 0, FW_NORMAL, FALSE, FALSE, FALSE,
                           DEFAULT_CHARSET, OUT_DEFAULT_PRECIS,
                           CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY,
                           DEFAULT_PITCH | FF_DONTCARE, L"Microsoft YaHei UI");
        preeditFont =
            CreateFontW(-kPreeditFontSize, 0, 0, 0, FW_NORMAL, FALSE, FALSE,
                        FALSE, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS,
                        CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY,
                        DEFAULT_PITCH | FF_DONTCARE, L"Microsoft YaHei UI");
        hintFont = CreateFontW(-13, 0, 0, 0, FW_NORMAL, FALSE, FALSE, FALSE,
                               DEFAULT_CHARSET, OUT_DEFAULT_PRECIS,
                               CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY,
                               DEFAULT_PITCH | FF_DONTCARE, L"Microsoft YaHei UI");
    }

    void layout() {
        if (!hwnd) {
            return;
        }
        HDC hdc = GetDC(hwnd);
        int width = 80;
        if (!snap.preedit.empty()) {
            width = std::max(width, textWidth(hdc, toWide(snap.preedit),
                                              kPreeditFontSize, preeditFont) +
                                        kPaddingX * 2);
        }
        if (snap.horizontal) {
            int total = kPaddingX * 2;
            for (size_t i = 0; i < snap.candidates.size(); ++i) {
                total += textWidth(hdc, toWide(std::to_string(i + 1) + "." +
                                               snap.candidates[i]),
                                   kFontSize, font) +
                         kItemGap;
            }
            if (!snap.candidates.empty()) {
                total -= kItemGap;
            }
            width = std::max(width, total);
        } else {
            int maxItem = 0;
            for (size_t i = 0; i < snap.candidates.size(); ++i) {
                maxItem = std::max(
                    maxItem,
                    textWidth(hdc, toWide(std::to_string(i + 1) + "." +
                                          snap.candidates[i]),
                              kFontSize, font));
            }
            width = std::max(width, maxItem + kPaddingX * 2);
        }
        int height = kPaddingY * 2;
        if (!snap.preedit.empty()) {
            height += kPreeditHeight;
        }
        if (!snap.candidates.empty()) {
            height += snap.horizontal
                          ? kItemHeight
                          : kItemHeight * (int)snap.candidates.size();
        }
        if (snap.hasPrev || snap.hasNext) {
            height += 16;
        }
        ReleaseDC(hwnd, hdc);

        // 屏幕内夹紧
        RECT work{};
        SystemParametersInfoW(SPI_GETWORKAREA, 0, &work, 0);
        int x = std::clamp<int>(pos.x, (int)work.left, std::max<int>((int)work.left, (int)work.right - width));
        int y = std::clamp<int>(pos.y, (int)work.top, std::max<int>((int)work.top, (int)work.bottom - height));
        SetWindowPos(hwnd, HWND_TOPMOST, x, y, width, height,
                     SWP_NOACTIVATE | SWP_SHOWWINDOW);
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        InvalidateRect(hwnd, nullptr, TRUE);
    }

    void paint() {
        PAINTSTRUCT ps{};
        HDC hdc = BeginPaint(hwnd, &ps);
        RECT rc{};
        GetClientRect(hwnd, &rc);

        HBRUSH bg = CreateSolidBrush(RGB(255, 255, 255));
        FillRect(hdc, &rc, bg);
        DeleteObject(bg);

        HPEN pen = CreatePen(PS_SOLID, 1, RGB(190, 180, 170));
        HGDIOBJ oldPen = SelectObject(hdc, pen);
        Rectangle(hdc, rc.left, rc.top, rc.right, rc.bottom);
        SelectObject(hdc, oldPen);
        DeleteObject(pen);

        SetBkMode(hdc, TRANSPARENT);
        int y = kPaddingY;
        if (!snap.preedit.empty()) {
            SelectObject(hdc, preeditFont);
            SetTextColor(hdc, RGB(110, 110, 110));
            auto w = toWide(snap.preedit);
            TextOutW(hdc, kPaddingX, y, w.data(), (int)w.size());
            y += kPreeditHeight;
        }
        if (!snap.candidates.empty()) {
            SelectObject(hdc, font);
            if (snap.horizontal) {
                int x = kPaddingX;
                for (size_t i = 0; i < snap.candidates.size(); ++i) {
                    auto label = toWide(std::to_string(i + 1) + "." +
                                        snap.candidates[i]);
                    if ((int)i == snap.cursor) {
                        HBRUSH hl = CreateSolidBrush(RGB(255, 232, 208));
                        RECT hr{x - 3, y - 2, x + textWidth(hdc, label, kFontSize, font) + 3,
                                y + kItemHeight};
                        FillRect(hdc, &hr, hl);
                        DeleteObject(hl);
                    }
                    SetTextColor(hdc, RGB(32, 32, 32));
                    TextOutW(hdc, x, y, label.data(), (int)label.size());
                    x += textWidth(hdc, label, kFontSize, font) + kItemGap;
                }
                y += kItemHeight;
            } else {
                for (size_t i = 0; i < snap.candidates.size(); ++i) {
                    auto label = toWide(std::to_string(i + 1) + "." +
                                        snap.candidates[i]);
                    if ((int)i == snap.cursor) {
                        HBRUSH hl = CreateSolidBrush(RGB(255, 232, 208));
                        RECT hr{kPaddingX - 3, y - 2,
                                kPaddingX + textWidth(hdc, label, kFontSize, font) + 3,
                                y + kItemHeight};
                        FillRect(hdc, &hr, hl);
                        DeleteObject(hl);
                    }
                    SetTextColor(hdc, RGB(32, 32, 32));
                    TextOutW(hdc, kPaddingX, y, label.data(), (int)label.size());
                    y += kItemHeight;
                }
            }
        }
        if (snap.hasPrev || snap.hasNext) {
            SelectObject(hdc, hintFont);
            SetTextColor(hdc, RGB(140, 140, 140));
            const wchar_t *hint = L"- = 翻页";
            TextOutW(hdc, kPaddingX, y, hint, (int)wcslen(hint));
        }
        EndPaint(hwnd, &ps);
    }

    void hide() {
        if (hwnd) {
            ShowWindow(hwnd, SW_HIDE);
        }
    }

    static LRESULT CALLBACK wndProc(HWND hwnd, UINT msg, WPARAM wParam,
                                    LPARAM lParam) {
        WinWindow *self = nullptr;
        if (msg == WM_NCCREATE) {
            auto *cs = reinterpret_cast<CREATESTRUCTW *>(lParam);
            self = static_cast<WinWindow *>(cs->lpCreateParams);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA,
                              reinterpret_cast<LONG_PTR>(self));
            self->hwnd = hwnd;
        } else {
            self = reinterpret_cast<WinWindow *>(
                GetWindowLongPtrW(hwnd, GWLP_USERDATA));
        }
        if (!self) {
            return DefWindowProcW(hwnd, msg, wParam, lParam);
        }
        switch (msg) {
        case WM_PAINT:
            self->paint();
            return 0;
        case WM_MOUSEACTIVATE:
            return MA_NOACTIVATE;
        default:
            return DefWindowProcW(hwnd, msg, wParam, lParam);
        }
    }
};

class WinUiFactory;

} // namespace

WinUi::WinUi(Instance *instance) : instance_(instance) {
    auto *win = new WinWindow();
    win->create();
    win_ = win;
}

WinUi::~WinUi() { delete static_cast<WinWindow *>(win_); }

void WinUi::update(UserInterfaceComponent component, InputContext *ic) {
    if (component != UserInterfaceComponent::InputPanel || suspended_) {
        return;
    }
    showPanel(ic);
}

bool WinUi::available() { return true; }

void WinUi::suspend() {
    suspended_ = true;
    hidePanel();
}

void WinUi::resume() { suspended_ = false; }

void WinUi::showPanel(InputContext *ic) {
    auto *win = static_cast<WinWindow *>(win_);
    win->snap = snapshotPanel(ic);
    if (win->snap.empty()) {
        win->hide();
        return;
    }
    win->pos.x = ic->cursorRect().left();
    win->pos.y = ic->cursorRect().top();
    win->layout();
}

void WinUi::hidePanel() { static_cast<WinWindow *>(win_)->hide(); }

class WinUiFactory : public AddonFactory {
public:
    AddonInstance *create(AddonManager *manager) override {
        return new WinUi(manager->instance());
    }
};

} // namespace fcitx

FCITX_ADDON_FACTORY_V2(winui, fcitx::WinUiFactory);

#include "clipboard_copy.h"

#ifdef _WIN32
// Windows 移植：直接用系统剪贴板 API（原版走 wl-copy/xclip/xsel 子进程）。
#include <windows.h>

namespace wowtype {

namespace {
constexpr std::size_t kMaxBytes = 32 * 1024;
} // namespace

bool copyToClipboard(const std::string &text) {
    if (text.empty() || text.size() > kMaxBytes) {
        return false;
    }
    const int wideLen = MultiByteToWideChar(CP_UTF8, 0, text.c_str(),
                                            static_cast<int>(text.size()),
                                            nullptr, 0);
    if (wideLen <= 0) {
        return false;
    }
    std::wstring wide(wideLen, L'\0');
    MultiByteToWideChar(CP_UTF8, 0, text.c_str(),
                        static_cast<int>(text.size()), wide.data(), wideLen);

    bool ok = false;
    // 剪贴板可能被其他程序占用，重试几次（TSF 场景下很短）
    for (int attempt = 0; attempt < 5 && !ok; ++attempt) {
        if (!OpenClipboard(nullptr)) {
            Sleep(20);
            continue;
        }
        if (EmptyClipboard()) {
            const SIZE_T bytes =
                (wide.size() + 1) * sizeof(wchar_t);
            HGLOBAL mem = GlobalAlloc(GMEM_MOVEABLE, bytes);
            if (mem != nullptr) {
                void *dst = GlobalLock(mem);
                if (dst != nullptr) {
                    memcpy(dst, wide.c_str(), bytes);
                    GlobalUnlock(mem);
                    ok = SetClipboardData(CF_UNICODETEXT, mem) != nullptr;
                }
                if (!ok) {
                    GlobalFree(mem);
                }
            }
        }
        CloseClipboard();
        if (!ok) {
            Sleep(20);
        }
    }
    return ok;
}

} // namespace wowtype

#else

#include <cerrno>
#include <fcntl.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>

namespace wowtype {

namespace {

// 先用 Wayland 的 wl-copy，再 X11 的 xclip、xsel。文字从标准输入读。
constexpr const char *kScript =
    "if [ -n \"$WAYLAND_DISPLAY\" ] && command -v wl-copy >/dev/null 2>&1; then exec wl-copy; fi\n"
    "if [ -n \"$DISPLAY\" ] && command -v xclip >/dev/null 2>&1; then"
    " exec xclip -selection clipboard; fi\n"
    "if [ -n \"$DISPLAY\" ] && command -v xsel >/dev/null 2>&1; then"
    " exec xsel --clipboard --input; fi\n"
    "exit 127\n";

// 管道一次写得下就不会卡住主线程（Linux 默认 64 KB）；语音一句话远远用不了这么多
constexpr std::size_t kMaxBytes = 32 * 1024;

} // namespace

bool copyToClipboard(const std::string &text) {
    if (text.empty() || text.size() > kMaxBytes) {
        return false;
    }
    int fds[2];
    if (pipe2(fds, O_CLOEXEC) != 0) {
        return false;
    }
    const int devnull = open("/dev/null", O_WRONLY | O_CLOEXEC);
    if (devnull < 0) {
        close(fds[0]);
        close(fds[1]);
        return false;
    }
    // 两次 fork：中间那个马上退出，真正干活的进程交给 init 收，fcitx 不用管它
    const pid_t child = fork();
    if (child == 0) {
        // fork 之后只调用异步信号安全的函数
        setsid();
        if (fork() == 0) {
            dup2(fds[0], STDIN_FILENO);
            dup2(devnull, STDOUT_FILENO);
            dup2(devnull, STDERR_FILENO);
            execl("/bin/sh", "sh", "-c", kScript, static_cast<char *>(nullptr));
            _exit(127);
        }
        _exit(0);
    }
    close(fds[0]);
    close(devnull);
    bool ok = child > 0;
    if (ok) {
        std::size_t done = 0;
        while (done < text.size()) {
            const ssize_t n = write(fds[1], text.data() + done, text.size() - done);
            if (n < 0 && errno == EINTR) {
                continue;
            }
            if (n <= 0) {
                ok = false;
                break;
            }
            done += static_cast<std::size_t>(n);
        }
    }
    close(fds[1]);
    if (child > 0) {
        while (waitpid(child, nullptr, 0) < 0 && errno == EINTR) {
        }
    }
    return ok;
}

} // namespace wowtype

#endif

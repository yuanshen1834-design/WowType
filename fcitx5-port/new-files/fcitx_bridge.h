/*
 * WowType Windows 移植：TSF ↔ fcitx5 引擎桥的共享定义。
 * 所有需要访问 bridge_ 成员的 .cpp 都 include 这个头。
 * SPDX-License-Identifier: LGPL-2.1-or-later
 */
#pragma once

#include <fcitx/inputcontext.h>
#include <fcitx/inputcontextmanager.h>
#include <fcitx/instance.h>
#include <fcitx-utils/eventdispatcher.h>
#include <fcitx-utils/event.h>

#include <atomic>
#include <condition_variable>
#include <memory>
#include <mutex>
#include <thread>

namespace fcitx {

class Tsf;

// TSF 专用的 InputContext：commit 直接进 TSF 上屏队列
class TsfInputContext : public InputContext {
  public:
    TsfInputContext(InputContextManager &manager, Tsf *owner);
    const char *frontend() const override { return "tsf"; }

  protected:
    void commitStringImpl(const std::string &text) override;
    void updatePreeditImpl() override {}
    void deleteSurroundingTextImpl(int offset, unsigned int size) override {}
    void forwardKeyImpl(const ForwardKeyEvent &key) override {}

  private:
    Tsf *owner_;
};

struct Tsf::EngineBridge {
    Tsf *owner = nullptr;
    std::unique_ptr<Instance> instance;
    std::unique_ptr<EventDispatcher> dispatcher;
    std::thread eventLoopThread;
    std::atomic<bool> ready{false};
    TsfInputContext *ic = nullptr;

    void start(Tsf *self);
    void startAsync() {
        eventLoopThread = std::thread(&EngineBridge::start, this, owner);
    }
    void stop();

    // 在事件循环线程执行 f；阻塞等待完成。
    template <typename F> void runOnLoop(F &&f) {
        if (!ready.load() || !dispatcher) {
            return;
        }
        bool done = false;
        std::mutex m;
        std::condition_variable cv;
        dispatcher->schedule([&]() {
            f();
            std::lock_guard<std::mutex> lk(m);
            done = true;
            cv.notify_all();
        });
        std::unique_lock<std::mutex> lk(m);
        cv.wait(lk, [&]() { return done; });
    }
};

} // namespace fcitx

/*
 * 最小 ATL 兼容层：只提供 fcitx5-windows 用到的 CComPtr / CComQIPtr。
 * MSYS2 clang64 没有 ATL（MSVC 专有），这里按相同语义实现子集。
 * SPDX-License-Identifier: LGPL-2.1-or-later
 */
#pragma once

#include <msctf.h>
#include <unknwn.h>

// mingw 的 msctf.h 缺这些（老版本 SDK 头），按 Windows SDK 补：
#ifndef TF_CLIENTID_NULL
#define TF_CLIENTID_NULL 0xFFFFFFFF
#endif

// ITfTextInputProcessorEx：IID {6E4E2102-F9CD-433D-B496-303CE03A6507}
#ifndef __ITfTextInputProcessorEx_INTERFACE_DEFINED__
#define __ITfTextInputProcessorEx_INTERFACE_DEFINED__
interface __declspec(uuid("6E4E2102-F9CD-433D-B496-303CE03A6507"))
    ITfTextInputProcessorEx : public ITfTextInputProcessor {
  public:
    virtual HRESULT STDMETHODCALLTYPE ActivateEx(ITfThreadMgr *pThreadMgr,
                                                 TfClientId tfClientId,
                                                 DWORD dwFlags) = 0;
};
#endif

// mingw msctf.h 缺的类别 GUID（值取自 Windows SDK / windows crate）。
// 用 static const 直接带值，不依赖 uuid.lib 的符号。
#ifndef GUID_TFCAT_CATEGORY_OF_TIP
static const GUID GUID_TFCAT_CATEGORY_OF_TIP = {
    0x534c48c1, 0x0607, 0x4098, {0xa5, 0x21, 0x4f, 0xc8, 0x99, 0xc7, 0x3e, 0x90}};
#endif
#ifndef GUID_TFCAT_TIPCAP_SECUREMODE
static const GUID GUID_TFCAT_TIPCAP_SECUREMODE = {
    0x49d2f9ce, 0x1f5e, 0x11d7, {0xa6, 0xd3, 0x00, 0x06, 0x5b, 0x84, 0x43, 0x5c}};
#endif
#ifndef GUID_TFCAT_TIPCAP_UIELEMENTENABLED
static const GUID GUID_TFCAT_TIPCAP_UIELEMENTENABLED = {
    0x49d2f9cf, 0x1f5e, 0x11d7, {0xa6, 0xd3, 0x00, 0x06, 0x5b, 0x84, 0x43, 0x5c}};
#endif
#ifndef GUID_TFCAT_TIPCAP_INPUTMODECOMPARTMENT
static const GUID GUID_TFCAT_TIPCAP_INPUTMODECOMPARTMENT = {
    0xccf05dd8, 0x4a87, 0x11d7, {0xa6, 0xe2, 0x00, 0x06, 0x5b, 0x84, 0x43, 0x5c}};
#endif
#ifndef GUID_TFCAT_TIPCAP_COMLESS
static const GUID GUID_TFCAT_TIPCAP_COMLESS = {
    0x364215d9, 0x75bc, 0x11d7, {0xa6, 0xef, 0x00, 0x06, 0x5b, 0x84, 0x43, 0x5c}};
#endif
#ifndef GUID_TFCAT_TIPCAP_WOW16
static const GUID GUID_TFCAT_TIPCAP_WOW16 = {
    0x364215da, 0x75bc, 0x11d7, {0xa6, 0xef, 0x00, 0x06, 0x5b, 0x84, 0x43, 0x5c}};
#endif
#ifndef GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT
static const GUID GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT = {
    0x13a016df, 0x560b, 0x46cd, {0x94, 0x7a, 0x4c, 0x3a, 0xf1, 0xe0, 0xe3, 0x5d}};
#endif
#ifndef GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT
static const GUID GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT = {
    0x25504fb4, 0x7bab, 0x4bc1, {0x9c, 0x69, 0xcf, 0x81, 0x89, 0x0f, 0x0e, 0xf5}};
#endif

namespace fcitx {

template <typename T> class CComPtr {
  public:
    CComPtr() noexcept = default;
    CComPtr(T *p) noexcept : p_(p) {}
    CComPtr(const CComPtr &other) noexcept : p_(other.p_) {
        if (p_) {
            p_->AddRef();
        }
    }
    CComPtr(CComPtr &&other) noexcept : p_(other.p_) { other.p_ = nullptr; }
    ~CComPtr() {
        if (p_) {
            p_->Release();
        }
    }
    CComPtr &operator=(const CComPtr &other) noexcept {
        if (other.p_) {
            other.p_->AddRef();
        }
        if (p_) {
            p_->Release();
        }
        p_ = other.p_;
        return *this;
    }
    CComPtr &operator=(T *other) noexcept {
        if (other) {
            other->AddRef();
        }
        if (p_) {
            p_->Release();
        }
        p_ = other;
        return *this;
    }
    T *operator->() const noexcept { return p_; }
    operator T *() const noexcept { return p_; }
    T **operator&() noexcept {
        release();
        return &p_;
    }
    T *get() const noexcept { return p_; }
    T *Detach() noexcept {
        T *p = p_;
        p_ = nullptr;
        return p;
    }
    void Attach(T *p) noexcept {
        if (p_) {
            p_->Release();
        }
        p_ = p;
    }
    void Release() noexcept {
        if (p_) {
            p_->Release();
            p_ = nullptr;
        }
    }
    void release() noexcept { Release(); }
    explicit operator bool() const noexcept { return p_ != nullptr; }

    // ATL 风格：ptr.CoCreateInstance(CLSID_xxx)
    HRESULT CoCreateInstance(REFCLSID rclsid, LPUNKNOWN pUnkOuter = nullptr,
                             DWORD dwClsContext = CLSCTX_INPROC_SERVER,
                             REFIID riid = __uuidof(T)) noexcept {
        T *result = nullptr;
        HRESULT hr = ::CoCreateInstance(rclsid, pUnkOuter, dwClsContext, riid,
                                        reinterpret_cast<void **>(&result));
        if (SUCCEEDED(hr) && result) {
            Release();
            p_ = result;
        }
        return hr;
    }

    // 常见用法：someInterface->QueryInterface(&ptr)
    template <typename U> HRESULT QueryInterface(CComPtr<U> *pp) const noexcept {
        if (!p_) {
            return E_POINTER;
        }
        return p_->QueryInterface(__uuidof(U), reinterpret_cast<void **>(pp));
    }

  private:
    T *p_ = nullptr;
};

template <typename T> using CComQIPtr = CComPtr<T>;

} // namespace fcitx

// ITfTextInputProcessorEx 的 IID（mingw 不生成；值来自 Windows SDK）
#ifndef __IID_ITfTextInputProcessorEx_DEFINED__
#define __IID_ITfTextInputProcessorEx_DEFINED__
static const GUID IID_ITfTextInputProcessorEx = {
    0x6e4e2102, 0xf9cd, 0x433d, {0xb4, 0x96, 0x30, 0x3c, 0xe0, 0x3a, 0x65, 0x07}};
#endif
#ifndef GUID_TFCAT_DISPLAYATTRIBUTEPROPERTY
static const GUID GUID_TFCAT_DISPLAYATTRIBUTEPROPERTY = {
    0xb95f181b, 0xea4c, 0x4af1, {0x80, 0x56, 0x7c, 0x32, 0x1a, 0xbb, 0xb0, 0x91}};
#endif
//! macOS 原生实现：
//! - 键盘模拟：CoreGraphics CGEvent（⌘C，合成键盘事件）
//! - 辅助功能权限检测：ApplicationServices AXIsProcessTrusted()
//! - Dock 图标显隐：NSApplication setActivationPolicy（菜单栏应用形态）
//! 全部为手写 FFI，直接链接系统框架，不引入第三方依赖。

use std::ffi::{c_char, c_void};

type CGEventRef = *mut c_void;
type CGEventSourceRef = *mut c_void;
type CGEventTapLocation = u32;
type CGKeyCode = u16;
type CGEventFlags = u64;

const K_CG_SESSION_EVENT_TAP: CGEventTapLocation = 1;
const K_CG_EVENT_FLAG_MASK_COMMAND: CGEventFlags = 1 << 20;
/// kVK_ANSI_C
const K_VK_ANSI_C: CGKeyCode = 8;

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventCreateKeyboardEvent(
        source: CGEventSourceRef,
        virtual_key: CGKeyCode,
        key_down: bool,
    ) -> CGEventRef;
    fn CGEventSetFlags(event: CGEventRef, flags: CGEventFlags);
    fn CGEventPost(tap: CGEventTapLocation, event: CGEventRef);
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRelease(cf: *const c_void);
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> u8;
}

#[link(name = "objc")]
extern "C" {
    fn objc_getClass(name: *const c_char) -> *mut c_void;
    fn sel_registerName(name: *const c_char) -> *mut c_void;
    fn objc_msgSend(receiver: *mut c_void, sel: *mut c_void, ...) -> *mut c_void;
}

/// 向当前前台应用合成一次 ⌘C 按键事件。
pub fn simulate_copy() -> Result<(), String> {
    if !accessibility_trusted() {
        return Err(
            "缺少辅助功能权限：请在系统设置 > 隐私与安全性 > 辅助功能 中允许 Word Collector"
                .into(),
        );
    }
    unsafe {
        let key_down = CGEventCreateKeyboardEvent(std::ptr::null_mut(), K_VK_ANSI_C, true);
        if key_down.is_null() {
            return Err("创建键盘事件失败（CGEventCreateKeyboardEvent 返回空）".into());
        }
        CGEventSetFlags(key_down, K_CG_EVENT_FLAG_MASK_COMMAND);
        CGEventPost(K_CG_SESSION_EVENT_TAP, key_down);
        CFRelease(key_down);

        let key_up = CGEventCreateKeyboardEvent(std::ptr::null_mut(), K_VK_ANSI_C, false);
        if key_up.is_null() {
            return Err("创建键盘事件失败（CGEventCreateKeyboardEvent 返回空）".into());
        }
        CGEventSetFlags(key_up, K_CG_EVENT_FLAG_MASK_COMMAND);
        CGEventPost(K_CG_SESSION_EVENT_TAP, key_up);
        CFRelease(key_up);
    }
    Ok(())
}

/// 是否已被授予“辅助功能”（Accessibility）权限。
/// 未授权时 CGEventPost 无法向其他应用派发合成事件。
pub fn accessibility_trusted() -> bool {
    unsafe { AXIsProcessTrusted() != 0 }
}

/// 打开 系统设置 > 隐私与安全性 > 辅助功能。
pub fn open_accessibility_settings() -> Result<(), String> {
    std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("无法打开系统设置: {e}"))
}

/// 切换 Dock 图标显示：隐藏设置窗口后转为纯菜单栏应用，
/// 从托盘重新打开设置窗口时恢复 Dock 图标。
pub fn set_dock_icon_visible(visible: bool) {
    unsafe {
        let cls = objc_getClass(b"NSApplication\0".as_ptr().cast::<c_char>());
        if cls.is_null() {
            return;
        }
        let sel_shared = sel_registerName(b"sharedApplication\0".as_ptr().cast::<c_char>());
        let app: *mut c_void = objc_msgSend(cls, sel_shared);
        if app.is_null() {
            return;
        }
        // NSApplicationActivationPolicyRegular = 0, .Accessory = 1
        let policy: i64 = if visible { 0 } else { 1 };
        let sel_policy = sel_registerName(b"setActivationPolicy:\0".as_ptr().cast::<c_char>());
        let _: *mut c_void = objc_msgSend(app, sel_policy, policy);
    }
}

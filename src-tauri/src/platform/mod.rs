//! 平台能力分发层：键盘模拟、辅助功能权限、辅助设置入口。
//! 各平台真实实现位于 `macos` / `windows` 子模块，均为 OS 原生 API。

#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "windows")]
pub mod windows;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessibilityStatus {
    Granted,
    Denied,
    /// 仅在非 macOS/Windows 平台编译时出现。
    #[allow(dead_code)]
    Unsupported,
}

/// 模拟用户在当前前台应用中按下复制快捷键（macOS: ⌘C，Windows: Ctrl+C）。
pub fn simulate_copy() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        return macos::simulate_copy();
    }
    #[cfg(target_os = "windows")]
    {
        return windows::simulate_copy();
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Err("当前平台不支持键盘模拟".into())
    }
}

/// macOS 辅助功能权限状态（Windows 上始终视为已授权）。
pub fn accessibility_status() -> AccessibilityStatus {
    #[cfg(target_os = "macos")]
    {
        if macos::accessibility_trusted() {
            AccessibilityStatus::Granted
        } else {
            AccessibilityStatus::Denied
        }
    }
    #[cfg(target_os = "windows")]
    {
        AccessibilityStatus::Granted
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        AccessibilityStatus::Unsupported
    }
}

pub fn accessibility_granted() -> bool {
    accessibility_status() == AccessibilityStatus::Granted
}

/// 打开系统权限设置页（macOS 跳到“隐私与安全性 > 辅助功能”）。
pub fn open_accessibility_settings() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        return macos::open_accessibility_settings();
    }
    #[cfg(target_os = "windows")]
    {
        Ok(())
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Ok(())
    }
}

//! Windows 原生实现：通过 user32.dll SendInput 合成 Ctrl+C 键盘事件。
//! 剪贴板读写由 arboard（内部为 win32 剪贴板 API）完成。

use winapi::um::winuser::{
    GetAsyncKeyState, SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_CONTROL,
};

/// 'C' 的虚拟键码 (VK_C = 0x43)。
const VK_C: u16 = 0x43;

fn key_input(vk: u16, flags: u32) -> INPUT {
    let mut input: INPUT = unsafe { std::mem::zeroed() };
    input.type_ = INPUT_KEYBOARD;
    unsafe {
        let ki = input.u.ki_mut();
        *ki = KEYBDINPUT {
            wVk: vk,
            wScan: 0,
            dwFlags: flags,
            time: 0,
            dwExtraInfo: 0,
        };
    }
    input
}

fn send_one(input: &INPUT) -> Result<(), String> {
    let sent = unsafe { SendInput(1, [std::ptr::read(input)].as_mut_ptr(), size_of_input()) };
    if sent != 1 {
        return Err("SendInput 注入键盘事件失败".into());
    }
    Ok(())
}

fn size_of_input() -> i32 {
    std::mem::size_of::<INPUT>() as i32
}

fn key_is_down(vk: i32) -> bool {
    // GetAsyncKeyState 最高位 = 当前物理按下状态。
    (unsafe { GetAsyncKeyState(vk) } as u16 & 0x8000) != 0
}

/// 等待用户松开快捷键的物理修饰键（Ctrl/Alt）。
/// 全局快捷键在按下组合键的瞬间就会触发采集，若不等待松开，
/// 注入的 C 键会与仍被按住的 Alt 组成 Ctrl+Alt+C，导致目标应用不执行复制。
/// 最多等待 300ms，超时则照常注入（不永久卡住管线）。
fn wait_modifiers_released() -> usize {
    for i in 0..30 {
        if !key_is_down(VK_CONTROL as i32) && !key_is_down(winapi::um::winuser::VK_MENU as i32) {
            return i * 10;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    300
}

/// 向当前前台应用合成一次 Ctrl+C。
/// 各按键之间留出短暂间隔，保证大多数应用都能识别复合键。
pub fn simulate_copy() -> Result<(), String> {
    let diag = std::env::var("WORD_COLLECTOR_DIAG").is_ok();

    if diag {
        use winapi::um::winuser::{GetForegroundWindow, GetWindowTextW};
        unsafe {
            let hwnd = GetForegroundWindow();
            let mut buf = [0u16; 256];
            let len = GetWindowTextW(hwnd, buf.as_mut_ptr(), 256);
            eprintln!(
                "[diag] foreground={:p} title={:?}",
                hwnd,
                String::from_utf16_lossy(&buf[..len as usize])
            );
        }
    }

    let waited_ms = wait_modifiers_released();
    if diag && waited_ms > 0 {
        eprintln!("[diag] waited {waited_ms}ms for Ctrl/Alt release before injecting");
    }

    let ctrl_down = key_input(VK_CONTROL as u16, 0);
    let c_down = key_input(VK_C, 0);
    let c_up = key_input(VK_C, KEYEVENTF_KEYUP);
    let ctrl_up = key_input(VK_CONTROL as u16, KEYEVENTF_KEYUP);

    send_one(&ctrl_down)?;
    std::thread::sleep(std::time::Duration::from_millis(20));
    send_one(&c_down)?;
    std::thread::sleep(std::time::Duration::from_millis(20));
    send_one(&c_up)?;
    std::thread::sleep(std::time::Duration::from_millis(20));
    send_one(&ctrl_up)?;
    Ok(())
}

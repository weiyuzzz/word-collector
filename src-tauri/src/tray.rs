//! 系统托盘 / macOS 菜单栏图标：真实的原生托盘（非窗口模拟）。
//! 提供 打开设置 / 暂停与恢复采集 / 退出。

use std::sync::Mutex;

use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager};

use crate::commands;

/// 托盘“暂停采集”勾选项的句柄，用于动态更新勾选状态。
pub struct TrayPauseItem(pub Mutex<Option<CheckMenuItem<tauri::Wry>>>);

pub fn create_tray(app: AppHandle) -> tauri::Result<()> {
    let open_settings = MenuItem::with_id(&app, "open-settings", "打开设置", true, None::<&str>)?;
    let pause =
        CheckMenuItem::with_id(&app, "toggle-pause", "暂停采集", true, false, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(&app)?;
    let quit = MenuItem::with_id(&app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(&app, &[&open_settings, &pause, &separator, &quit])?;

    app.manage(TrayPauseItem(Mutex::new(Some(pause))));

    let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))
        .expect("内置托盘图标解码失败");

    TrayIconBuilder::with_id("word-collector-tray")
        .icon(icon)
        .icon_as_template(cfg!(target_os = "macos"))
        .tooltip("Word Collector — 选中单词后按全局快捷键采集")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open-settings" => commands::show_settings(app),
            "toggle-pause" => {
                let paused = commands::toggle_pause(app);
                let tray = app.state::<TrayPauseItem>();
                let guard = tray.0.lock().unwrap();
                if let Some(item) = guard.as_ref() {
                    let _ = item.set_checked(paused);
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .build(&app)?;
    Ok(())
}

/// 供暂停状态同步使用（托盘勾选随暂停状态更新）。
#[allow(dead_code)]
pub fn sync_pause_state(app: &AppHandle, paused: bool) {
    let tray = app.state::<TrayPauseItem>();
    let guard = tray.0.lock().unwrap();
    if let Some(item) = guard.as_ref() {
        let _ = item.set_checked(paused);
    }
}

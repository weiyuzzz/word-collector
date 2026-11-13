//! 运行时状态（与持久化配置分离）。

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;

use crate::config::Config;

pub struct AppState {
    pub config: Mutex<Config>,
    /// 采集暂停开关（托盘/设置界面均可切换）。
    paused: AtomicBool,
    /// 本次运行累计成功采集的单词数。
    collected: AtomicUsize,
    /// 最近一次采集失败的错误信息，用于设置界面展示。
    last_error: Mutex<Option<String>>,
    /// 采集互斥锁：防止连续触发快捷键时两个采集管线并发争抢剪贴板。
    capture_lock: Mutex<()>,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        Self {
            config: Mutex::new(config),
            paused: AtomicBool::new(false),
            collected: AtomicUsize::new(0),
            last_error: Mutex::new(None),
            capture_lock: Mutex::new(()),
        }
    }

    /// 获取采集互斥锁的守卫（在执行采集前持有）。
    pub fn acquire_capture_lock(&self) -> std::sync::MutexGuard<'_, ()> {
        self.capture_lock.lock().unwrap_or_else(|poisoned| {
            poisoned.into_inner()
        })
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }

    pub fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::SeqCst);
    }

    pub fn record_collected(&self) -> usize {
        self.collected.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub fn collected(&self) -> usize {
        self.collected.load(Ordering::SeqCst)
    }

    pub fn set_last_error(&self, err: Option<String>) {
        *self.last_error.lock().unwrap() = err;
    }

    pub fn last_error(&self) -> Option<String> {
        self.last_error.lock().unwrap().clone()
    }
}

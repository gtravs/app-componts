#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_mut)]
use slint::{ComponentHandle, ModelRc, SharedString, VecModel, Weak, Global};
use std::sync::{Arc, Mutex};
use chrono::Local;
use tokio::sync::mpsc;
use tokio::time::{Duration, Instant};
use crate::slint_generatedApp::{self, LogEntry, GlobalBasicSettings};
use once_cell::sync::OnceCell;
use encoding_rs::{Encoding, UTF_8, GBK, GB18030};
// 全局日志管理器
static GLOBAL_LOG_MANAGER: OnceCell<Arc<LogManager>> = OnceCell::new();

#[derive(Clone, Debug)]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
}

impl LogLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warning => "WARN",
            LogLevel::Error => "ERROR",
        }
    }
}

#[derive(Clone)]
pub struct LogManager {
    sender: mpsc::Sender<LogEntry>,
}

impl LogManager {
    fn new(sender: mpsc::Sender<LogEntry>) -> Self {
        Self { sender }
    }

    pub async fn log(&self, message: impl Into<String>, level: LogLevel) -> Result<(), mpsc::error::SendError<LogEntry>> {
        let entry = LogEntry {
            timestamp: SharedString::from(Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string()),
            level: SharedString::from(level.as_str()),
            message: SharedString::from(message.into()),
        };
        self.sender.send(entry).await
    }
}

struct LogStorage {
    entries: Vec<LogEntry>,
}

impl LogStorage {
    fn new() -> Self {
        Self { entries: Vec::new() }
    }

    fn add_entry(&mut self, entry: LogEntry, max_logs: usize) {
        self.entries.push(entry);
        if self.entries.len() > max_logs {
            self.entries.remove(0);
        }
    }
}

#[derive(Clone)]
pub struct LogSystem<T> 
where
    T: ComponentHandle  + 'static,
{
    window: Weak<T>,
    sender: mpsc::Sender<LogEntry>,
    max_logs: usize,
    batch_size: usize,
    update_interval: Duration,
    storage: Arc<Mutex<LogStorage>>,
}

pub fn get_log_manager() -> Option<Arc<LogManager>> {
    GLOBAL_LOG_MANAGER.get().cloned()
}

impl<T> LogSystem<T>
where
    T: ComponentHandle  + 'static,
    for<'a> GlobalBasicSettings<'a>: Global<'a, T>,
{
    pub fn new(
        window: &T,
        max_logs: usize,
        batch_size: usize,
        update_interval: Duration,
    ) -> (Arc<LogManager>, Self) 
    {
        let (sender, mut receiver) = mpsc::channel::<LogEntry>(10000);
        let storage = Arc::new(Mutex::new(LogStorage::new()));
        let log_manager = Arc::new(LogManager::new(sender.clone()));
        GLOBAL_LOG_MANAGER.set(log_manager.clone()).ok();

        Self::initialize_ui_model(window);
        Self::spawn_log_handler(receiver, storage.clone(), window.as_weak(), max_logs);

        let system = Self {
            window: window.as_weak(),
            sender,
            max_logs,
            batch_size,
            update_interval,
            storage,
        };

        (log_manager, system)
    }

    fn initialize_ui_model(window: &T) 
    {
        let model = VecModel::default();
        let model_rc = ModelRc::new(model);
        let global_settings = window.global::<GlobalBasicSettings>();
        global_settings.set_logs(model_rc);
    }

    fn spawn_log_handler(
        mut receiver: mpsc::Receiver<LogEntry>,
        storage: Arc<Mutex<LogStorage>>,
        window_weak: Weak<T>,
        max_logs: usize,
    ) {
        tokio::spawn(async move {
            while let Some(entry) = receiver.recv().await {
                let entries_to_display = {
                    let mut storage = storage.lock().unwrap();
                    storage.add_entry(entry, max_logs);
                    storage.entries.clone()
                };
                
                Self::update_ui(window_weak.clone(), entries_to_display);
            }
        });
    }

    fn update_ui(window_weak: Weak<T>, entries: Vec<LogEntry>) {
        slint::invoke_from_event_loop(move || {
            if let Some(window) = window_weak.upgrade() {
                let model = VecModel::default();
                for entry in entries {
                    model.push(entry);
                }
                let model_rc = ModelRc::new(model);
                window.global::<GlobalBasicSettings>().set_logs(model_rc);
            }
        }).ok();
    }

    pub async fn log(&self, message: impl Into<String>, level: LogLevel) -> Result<(), mpsc::error::SendError<LogEntry>> {
        let message = message.into();
        let decoded_message = Self::decode_message(message.as_bytes());
        let entry = LogEntry {
            timestamp: SharedString::from(Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string()),
            level: SharedString::from(level.as_str()),
            message: SharedString::from(decoded_message),
        };
        self.sender.send(entry).await
    }

    fn decode_message(bytes: &[u8]) -> String {
        // 首先尝试 UTF-8
        if let Ok(text) = String::from_utf8(bytes.to_vec()) {
            return text;
        }
        // 尝试 GBK
        if let Some(decoded) = Encoding::for_label("gbk".as_bytes())
            .and_then(|encoding| {
                let (cow, _, had_errors) = encoding.decode(bytes);
                if !had_errors {
                    Some(cow.into_owned())
                } else {
                    None
                }
            }) {
            return decoded;
        }

        // 尝试 GB18030
        if let Some(decoded) = Encoding::for_label("gb18030".as_bytes())
            .and_then(|encoding| {
                let (cow, _, had_errors) = encoding.decode(bytes);
                if !had_errors {
                    Some(cow.into_owned())
                } else {
                    None
                }
            }) {
            return decoded;
        }

        // 如果都失败了，返回十六进制表示
        bytes.iter()
            .map(|b| format!("{:02X}", b))
            .collect::<Vec<String>>()
            .join(" ")
    }

    pub fn get_logs(&self) -> Vec<LogEntry> {
        self.storage.lock()
            .map(|storage| storage.entries.clone())
            .unwrap_or_default()
    }

    pub fn clear_logs(&self) {
        if let Ok(mut storage) = self.storage.lock() {
            storage.entries.clear();
            Self::update_ui(self.window.clone(), Vec::new());
        }
    }

    pub async fn batch_log(&self, entries: Vec<(String, LogLevel)>) -> Result<(), mpsc::error::SendError<LogEntry>> {
        for (message, level) in entries {
            self.log(message, level).await?;
        }
        Ok(())
    }
}

// 保持宏定义不变
#[macro_export]
macro_rules! log {
    ($level:expr, $($arg:tt)*) => {
        if let Some(logger) = $crate::logs::get_log_manager() {
            let message = format!($($arg)*);
            let logger = logger.clone();
            tokio::spawn(async move {
                logger.log(message, $level).await.ok();
            });
        }
    };
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => { $crate::log!($crate::logs::LogLevel::Info, $($arg)*) };
}

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => { $crate::log!($crate::logs::LogLevel::Debug, $($arg)*) };
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => { $crate::log!($crate::logs::LogLevel::Warning, $($arg)*) };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => { $crate::log!($crate::logs::LogLevel::Error, $($arg)*) };
}
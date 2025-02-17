#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_mut)]
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use slint::platform::EventLoopProxy;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel, Weak, Global};
use std::cmp::max;
use std::rc::Rc;
use std::sync::{Arc, Mutex, OnceLock};
use chrono::Local;
use tokio::sync::mpsc;
use tokio::time::{Duration, Instant};
use crate::slint_generatedApp::{self, GlobalBasicSettings};
use encoding_rs::{Encoding, UTF_8, GBK, GB18030};

use super::logs::{LogManager, LogStorage, LogSystem};
use super::logs_struct::ProxyLogEntry;


/*
    -  日志系统: LogSystem 
    -  日志管理器: LogManger
*/

pub static  GLOBAL_PROXY_MANAGER:OnceCell<Arc<ProxyLogManager>> = OnceCell::new();
pub fn get_log_manager() -> Option<Arc<ProxyLogManager>> {
    GLOBAL_PROXY_MANAGER.get().cloned()
}

// 日志管理器
#[derive(Clone)]
pub struct ProxyLogManager {
    sender : mpsc::Sender<ProxyLogEntry>
}

impl LogManager for ProxyLogManager {
    type Entry = ProxyLogEntry;
    fn new(sender: mpsc::Sender<ProxyLogEntry>) -> Self {
        Self{
            sender
        }
    }
    fn get_sender(&self) -> &mpsc::Sender<Self::Entry> {
        &self.sender
    }
    
    async fn log_entry(&self, entry: Self::Entry) -> Result<(), mpsc::error::SendError<Self::Entry>> {
        self.get_sender().send(entry).await
    }
    
    
}



#[derive(Clone)]
pub struct ProxyLogStorage {
    entries: Vec<ProxyLogEntry>
}

impl LogStorage for ProxyLogStorage {
    type Entry = ProxyLogEntry;
    fn new() -> Self  where Self: Sized {
        Self {
            entries: Vec::new()
        }
    }

    fn get_entries(&self) -> &Vec<Self::Entry> {
        &self.entries
    }

    fn get_entries_mut(&mut self) -> &mut Vec<Self::Entry> {
        &mut self.entries
    }
}

// 日志系统
pub struct ProxyLogSystem<T1>
where 
    T1: ComponentHandle  + 'static,
{
    pub window: Weak<T1>,
   pub sender: mpsc::Sender<ProxyLogEntry>,
   pub max_logs: usize,
   pub batch_size: usize,
   pub update_interval: Duration,
   pub storage: Arc<Mutex<ProxyLogStorage>>,
}

impl<T1>LogSystem<T1> for ProxyLogSystem<T1>
where 
    T1: ComponentHandle  + 'static,
    for<'a> GlobalBasicSettings<'a>: Global<'a, T1>,
{
    type Entry =  ProxyLogEntry;

    fn new(
        window: &T1,
        max_logs: usize,
        batch_size: usize,
        update_interval: Duration
    ) -> Self {
        let (sender, mut receiver) = mpsc::channel(1000);
        let storage = Arc::new(Mutex::new(ProxyLogStorage::new()));
        let log_manager = Arc::new(ProxyLogManager::new(sender.clone()));
        GLOBAL_PROXY_MANAGER.set(log_manager.clone()).ok();

        Self::init_ui_model(window);
        let st = Arc::clone(&storage);
        Self::setup_clear_callback(window, st);
        Self::spawn_log_handler(receiver, storage.clone(), window.as_weak(), max_logs);
        let system = Self {
            window: window.as_weak(),
            sender,
            max_logs,
            batch_size,
            update_interval,
            storage,
        };
        system
    }

    fn get_window(&self) -> &Weak<T1> {
        &self.window
    }

    fn get_sender(&self) -> &mpsc::Sender<Self::Entry> {
        &self.sender
    }

    fn get_storage(&self) -> &Arc<Mutex<dyn LogStorage<Entry = Self::Entry>>> {
        // 使用 as 进行转换
        unsafe {
            std::mem::transmute(&self.storage)
        }
    }

    fn get_max_logs(&self) -> usize {
        self.max_logs
    }

    fn get_batch_size(&self) -> usize {
        self.batch_size
    }

    fn get_update_interval(&self) -> Duration {
        self.update_interval
    }

    fn init_ui_model(window: &T1) {
        let global_settings = window.global::<GlobalBasicSettings>();
        let model= VecModel::default();
        let model_rc = ModelRc::new(model);
        global_settings.set_proxy_logs(model_rc);  
    }

    fn spawn_log_handler(mut receiver: mpsc::Receiver<Self::Entry>,storage: Arc<Mutex<dyn LogStorage<Entry = Self::Entry>>>,window_weak: Weak<T1>,max_logs: usize)  {
        tokio::spawn(
            async move {
                    //println!("recv data.");
                    while let Some(entry) = receiver.recv().await {
                        let entries_to_display:Vec<ProxyLogEntry> = {
                            let mut storage = storage.lock().unwrap();
                            storage.add_entry(entry, max_logs);
                            storage.get_entries().clone()
                        };
                        //println!("entry: {:?}", entries_to_display); 
                        Self::update_ui(window_weak.clone(), entries_to_display);
                   }
        });
    }
    fn update_ui(window_weak: Weak<T1>, entries: Vec<Self::Entry>) 
    {
        slint::invoke_from_event_loop(move || {
            //println!("Updating UI with {} entries", entries.len());
            if let Some(window) = window_weak.upgrade() {
                // window.global::<GlobalBasicSettings>().on_get_proxy_log(move || {
                // });  
                let global_settings = window.global::<GlobalBasicSettings>();
                let model= VecModel::default();
                for entry in entries {
                    let slint_entry = slint_generatedApp::ProxyLogEntry::from(entry);
                    model.push(slint_entry);
                }

                let model_rc = ModelRc::new(model);
                global_settings.set_proxy_logs(model_rc); 
            }
        }).ok();
    }

    fn setup_clear_callback(window: &T1, storage: Arc<Mutex<dyn LogStorage<Entry = Self::Entry>>>) {
        let window_weak = window.as_weak();
        let global_settings = window.global::<GlobalBasicSettings>();
        global_settings.on_clear_proxy_logs(move || {
            // 清空存储的日志
            if let Ok(mut storage) = storage.lock() {
                storage.get_entries_mut().clear();
            }
            // 更新UI
            let weak_clone = window_weak.clone();
            slint::invoke_from_event_loop(move || {
                if let Some(window) = weak_clone.upgrade() {
                    let empty_model = ModelRc::new(VecModel::default());
                    window.global::<GlobalBasicSettings>().set_proxy_logs(empty_model);
                }
            }).ok();
        });
    }
}

#[macro_export]
macro_rules! proxy_log {
    ($id:expr, $host:expr, $method:expr, $url:expr, $status:expr, $length:expr) => {
        if let Some(logger) = crate::logs::http_proxylogs::get_log_manager() {
            let entry = crate::logs::logs_struct::ProxyLogEntry {
                timestamp: chrono::Local::now().format("%Y-%m-%d %H:%M").to_string().into(),
                id: $id.into(),
                host: $host.into(),
                method: $method.into(),
                url: $url.into(),
                status_code: $status.into(),
                length: $length.into(),
            };
            let logger = logger.clone();
            tokio::spawn(async move {
                use crate::logs::logs::LogManager;
                logger.log_entry(entry).await.ok();
            });
        }
    };
}

#[macro_export]
macro_rules! proxy_info {
    ($id:expr, $host:expr, $method:expr, $url:expr, $status:expr, $length:expr) => {
        $crate::proxy_log!(
            $id,
            $host,
            $method,
            $url,
            $status,
            $length
        )
    };
}
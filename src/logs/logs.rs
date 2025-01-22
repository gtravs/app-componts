// #![allow(dead_code)]
// #![allow(unused_imports)]
// #![allow(unused_mut)]
// use once_cell::sync::OnceCell;
// use slint::platform::EventLoopProxy;
// use slint::{ComponentHandle, ModelRc, SharedString, VecModel, Weak, Global};
// use std::cmp::max;
// use std::rc::Rc;
// use std::sync::{Arc, Mutex, OnceLock};
// use chrono::Local;
// use tokio::sync::mpsc;
// use tokio::time::{Duration, Instant};
// use crate::slint_generatedApp::{self, GlobalBasicSettings,ProxyLogEntry};
// use encoding_rs::{Encoding, UTF_8, GBK, GB18030};
// use super::logs_struct::{LogEntry};


// /*
//     -  日志系统: LogSystem 
//     -  日志管理器: LogManger
// */

// pub static  GLOBAL_LOG_MANAGER:OnceCell<Arc<LogManager>> = OnceCell::new();
// pub fn get_log_manager() -> Option<Arc<LogManager>> {
//     GLOBAL_LOG_MANAGER.get().cloned()
// }


// // 日志管理器
// #[derive(Clone)]
// pub struct LogManager {
//     sender : mpsc::Sender<ProxyLogEntry>
// }

// impl LogManager {
//     pub fn new(sender: mpsc::Sender<ProxyLogEntry>) -> Self {
//         Self{
//             sender
//         }
//     }
//     pub async  fn log(&self, id: impl Into<String>,host: impl Into<String>,method: impl Into<String>,url: impl Into<String>,status_code: impl Into<String>,length: impl Into<String>) -> Result<(),mpsc::error::SendError<ProxyLogEntry>>   {     
//         let entry = ProxyLogEntry {
//             timestamp: SharedString::from(Local::now().format("%Y-%m-%d %H:%M").to_string()),
//             id: SharedString::from(id.into()),
//             host: SharedString::from(host.into()),
//             method: SharedString::from(method.into()),
//             url: SharedString::from(url.into()),
//             status_code: SharedString::from(status_code.into()),
//             length: SharedString::from(length.into()),
//         };
//         //println!("Logging: {:?}", entry); 
//         self.sender.send(entry).await
//     }
// }

// //  日志存储器
// #[derive(Clone)]
// pub  struct LogStorage {
//     entries: Vec<ProxyLogEntry>
// }

// impl LogStorage {
//     pub fn new() -> Self {
//         Self {  
//             entries:Vec::new()
//         }
//     }

//     pub fn add_entry(&mut self,entry:ProxyLogEntry,max_size:usize) {
//         self.entries.push(entry);
//         //println!("add_entry: {:?}", self.entries); 
//         if self.entries.len() > max_size {
//             self.entries.remove(0);
//         }
//     }

//     pub fn clear(&mut self) {
//         self.entries.clear();
//     }
// }

// // 日志系统
// pub struct LogSystem<T1>
// where 
//     T1: ComponentHandle  + 'static,
// {
//     pub window: Weak<T1>,
//    pub sender: mpsc::Sender<ProxyLogEntry>,
//    pub max_logs: usize,
//    pub batch_size: usize,
//    pub update_interval: Duration,
//    pub storage: Arc<Mutex<LogStorage>>,
// }

// impl<T1> LogSystem<T1>
// where 
//     T1: ComponentHandle  + 'static,
//     for<'a> GlobalBasicSettings<'a>: Global<'a, T1>,
// {
//     pub fn new(window:&T1,max_logs:usize,batch_size: usize,update_interval: Duration) -> Self{
//         let (sender, mut receiver) = mpsc::channel(1000);
//         let storage = Arc::new(Mutex::new(LogStorage::new()));
//         let log_manager = Arc::new(LogManager::new(sender.clone()));
//         GLOBAL_LOG_MANAGER.set(log_manager.clone()).ok();

//         Self::init_ui_model(window);
//         Self::setup_clear_callback(window, Arc::clone(&storage));
//         Self::spawn_log_handler(receiver, storage.clone(), window.as_weak(), max_logs);
//         let system = Self {
//             window: window.as_weak(),
//             sender,
//             max_logs,
//             batch_size,
//             update_interval,
//             storage,
//         };
//         system
//     }

//     pub fn init_ui_model(window:&T1) {
//         let global_settings = window.global::<GlobalBasicSettings>();
//         let model:VecModel<ProxyLogEntry>= VecModel::default();
//         let model_rc = ModelRc::new(model);
//         global_settings.set_proxy_logs(model_rc);  
//     }

//     pub fn spawn_log_handler(mut receiver: mpsc::Receiver<ProxyLogEntry>,storage: Arc<Mutex<LogStorage>>,window_weak: Weak<T1>,max_logs: usize)  {
//         tokio::spawn(
//             async move {
//                     //println!("recv data.");
//                     while let Some(entry) = receiver.recv().await {
//                         let entries_to_display:Vec<ProxyLogEntry> = {
//                             let mut storage = storage.lock().unwrap();
//                             storage.add_entry(entry, max_logs);
//                             storage.entries.clone()
//                         };
//                         //println!("entry: {:?}", entries_to_display); 
//                         Self::update_ui(window_weak.clone(), entries_to_display);
//                    }
//         });
//     }

//     fn update_ui(window_weak: Weak<T1>, entries: Vec<ProxyLogEntry>) {
//         slint::invoke_from_event_loop(move || {
//             //println!("Updating UI with {} entries", entries.len());
//             if let Some(window) = window_weak.upgrade() {
//                 // window.global::<GlobalBasicSettings>().on_get_proxy_log(move || {
//                 // });  
//                 let model:VecModel<ProxyLogEntry>= VecModel::default();
//                 for entry in entries {
//                     model.push(entry);
//                 }

//                 let model_rc = ModelRc::new(model);
//                 window.global::<GlobalBasicSettings>().set_proxy_logs(model_rc);
//             }
//         }).ok();
//     }

//     fn setup_clear_callback(window: &T1, storage: Arc<Mutex<LogStorage>>) {
//         let window_weak = window.as_weak();
//         let global_settings = window.global::<GlobalBasicSettings>();
//         global_settings.on_clear_proxy_logs(move || {
//             // 清空存储的日志
//             if let Ok(mut storage) = storage.lock() {
//                 storage.entries.clear();
//             }
//             // 更新UI
//             let weak_clone = window_weak.clone();
//             slint::invoke_from_event_loop(move || {
//                 if let Some(window) = weak_clone.upgrade() {
//                     let empty_model = ModelRc::new(VecModel::default());
//                     window.global::<GlobalBasicSettings>().set_proxy_logs(empty_model);
//                 }
//             }).ok();
//         });
//     }

// }


// // #[derive(Clone, Copy)]
// // pub enum LogLevel {
// //     Info,
// //     Debug,
// //     Warning,
// //     Error,
// // }

// // impl LogLevel {
// //     fn as_str(&self) -> &'static str {
// //         match self {
// //             LogLevel::Info => "INFO",
// //             LogLevel::Debug => "DEBUG",
// //             LogLevel::Warning => "WARN",
// //             LogLevel::Error => "ERROR",
// //         }
// //     }
// // }

// // #[macro_export]
// // macro_rules! info {
// //     ($id:expr, $host:expr, $method:expr, $url:expr, $status:expr, $length:expr) => {
// //         $crate::proxy_log!(
// //             $id,
// //             $host,
// //             $method,
// //             $url,
// //             $status,
// //             $length
// //         )
// //     };
// // }

// // #[macro_export]
// // macro_rules! debug {
// //     ($id:expr, $host:expr, $method:expr, $url:expr, $status:expr, $length:expr) => {
// //         $crate::proxy_log!(
// //             $id,
// //             $host,
// //             $method,
// //             $url,
// //             $status,
// //             $length
// //         )
// //     };
// // }

// // #[macro_export]
// // macro_rules! warn {
// //     ($id:expr, $host:expr, $method:expr, $url:expr, $status:expr, $length:expr) => {
// //         $crate::proxy_log!(
// //             $id,
// //             $host,
// //             $method,
// //             $url,
// //             $status,
// //             $length
// //         )
// //     };
// // }

// // #[macro_export]
// // macro_rules! error {
// //     ($id:expr, $host:expr, $method:expr, $url:expr, $status:expr, $length:expr) => {
// //         $crate::proxy_log!(
// //             $id,
// //             $host,
// //             $method,
// //             $url,
// //             $status,
// //             $length
// //         )
// //     };
// // }
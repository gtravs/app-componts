#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_mut)]
use once_cell::sync::OnceCell;
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


pub trait LogEntry: Default+Clone + Send + 'static {}

pub trait  LogManager {
    type Entry: LogEntry;
    fn new(sender: mpsc::Sender<Self::Entry>) -> Self;
    fn get_sender(&self) -> &mpsc::Sender<Self::Entry>;
    async fn log_entry(&self, entry: Self::Entry) -> Result<(), mpsc::error::SendError<Self::Entry>> {
        self.get_sender().send(entry).await
    }
}

pub trait LogStorage:Send {
    type Entry: LogEntry;
    fn new() -> Self  where Self: Sized;
    fn get_entries(&self) -> &Vec<Self::Entry>;
    fn get_entries_mut(&mut self) -> &mut Vec<Self::Entry>;
    fn add_entry(&mut self, entry: Self::Entry, max_size: usize) {
        let entries = self.get_entries_mut();
        entries.push(entry);
        if entries.len() > max_size {
            entries.remove(0);
        }
    }
    fn clear(&mut self) {
        self.get_entries_mut().clear();
    }
}

pub trait LogSystem<T1> 
where 
    T1: ComponentHandle  + 'static,
{
    type Entry: LogEntry;
    fn new(
        window: &T1,
        max_logs: usize,
        batch_size: usize,
        update_interval: Duration
    ) -> Self;
        // 新增：访问字段的方法
    fn get_window(&self) -> &Weak<T1>;
    fn get_sender(&self) -> &mpsc::Sender<Self::Entry>;
    fn get_storage(&self) -> &Arc<Mutex<dyn LogStorage<Entry = Self::Entry>>>;
    fn get_max_logs(&self) -> usize;
    fn get_batch_size(&self) -> usize;
    fn get_update_interval(&self) -> Duration;
    // 默认实现
    fn init_ui_model(window: &T1) 
    where for<'a> GlobalBasicSettings<'a>: Global<'a, T1>;
    fn spawn_log_handler(receiver: mpsc::Receiver<Self::Entry>,storage: Arc<Mutex<dyn LogStorage<Entry = Self::Entry>>>,window_weak: Weak<T1>,max_logs: usize);
    fn update_ui(window_weak: Weak<T1>, entries: Vec<Self::Entry>) 
    where for<'a> GlobalBasicSettings<'a>: Global<'a, T1>;
    fn setup_clear_callback(window: &T1, storage: Arc<Mutex<dyn LogStorage<Entry = Self::Entry>>>);
}
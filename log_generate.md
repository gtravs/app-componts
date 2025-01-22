Slint 多组件日志系统设计文档
1. 系统架构
1.1 核心组件
mermaid


graph TD
    A[LogEntry] --> B[LogDispatcher]
    B --> C1[ProxyLogHandler]
    B --> C2[NetworkLogHandler]
    B --> C3[SystemLogHandler]
    C1 --> D1[Proxy UI]
    C2 --> D2[Network UI]
    C3 --> D3[System UI]
1.2 数据结构
rust


#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum LogTarget {
    General,
    Proxy,
    Network,
    System,
    Custom(String),
}

#[derive(Clone, Debug)]
pub struct LogEntry {
    pub timestamp: SharedString,
    pub target: LogTarget,
    pub level: SharedString,
    pub message: SharedString,
}
2. 核心实现
2.1 日志分发器
rust


pub struct LogDispatcher<T: ComponentHandle + 'static> {
    window: Weak<T>,
    channels: Arc<DashMap<LogTarget, mpsc::Sender<LogEntry>>>,
    storages: Arc<DashMap<LogTarget, Arc<Mutex<Vec<LogEntry>>>>>,
    max_logs: usize,
}
2.2 日志处理器
rust


pub struct LogHandler<T: ComponentHandle + 'static> {
    window: Weak<T>,
    target: LogTarget,
    storage: Arc<Mutex<Vec<LogEntry>>>,
    batch_size: usize,
}
2.3 日志管理器
rust


pub struct LogManager<T: ComponentHandle + 'static> {
    dispatcher: LogDispatcher<T>,
    handlers: HashMap<LogTarget, LogHandler<T>>,
}
3. 接口定义
3.1 Slint UI 接口
slint


export global Logging {
    in property <[LogEntry]> general_logs;
    in property <[LogEntry]> proxy_logs;
    in property <[LogEntry]> network_logs;
    in property <[LogEntry]> system_logs;
}
3.2 日志发送接口
rust


impl LogManager {
    pub async fn log(&self, entry: LogEntry) -> Result<(), Box<dyn std::error::Error>>;
    pub async fn batch_log(&self, entries: Vec<LogEntry>) -> Result<(), Box<dyn std::error::Error>>;
}
4. 使用示例
4.1 初始化系统
rust


fn main() {
    let app = AppWindow::new().unwrap();
    let log_manager = LogManager::new(&app);
    
    // 启动异步日志处理
    tokio::spawn(async move {
        log_manager.start().await;
    });
    
    app.run().unwrap();
}
4.2 发送日志
rust


// 发送单条日志
log_manager.log(LogEntry {
    timestamp: Local::now().format("%Y-%m-%d %H:%M:%S").to_string().into(),
    target: LogTarget::Proxy,
    level: "INFO".into(),
    message: "代理启动成功".into(),
}).await?;

// 批量发送日志
log_manager.batch_log(vec![
    LogEntry { /* ... */ },
    LogEntry { /* ... */ },
]).await?;
5. 性能优化策略
5.1 批处理机制
使用批量更新减少UI刷新频率
设置合理的批处理大小（默认100条）
使用定时刷新确保实时性
5.2 内存管理
rust


impl LogHandler {
    fn clean_old_logs(&mut self) {
        let mut storage = self.storage.lock().unwrap();
        if storage.len() > self.max_logs {
            storage.drain(..storage.len() - self.max_logs);
        }
    }
}
5.3 并发处理
使用 DashMap 实现并发访问
异步处理所有IO操作
使用 Weak 引用避免内存泄漏
6. UI 组件实现
6.1 日志视图组件
slint


component LogView inherits VerticalBox {
    in property <[LogEntry]> logs;
    
    for log in logs : Rectangle {
        Text {
            text: "{log.timestamp} [{log.level}] {log.message}";
        }
    }
}
6.2 主窗口布局
slint


export component MainWindow inherits Window {
    VerticalBox {
        TabWidget {
            Tab {
                title: "General";
                LogView { logs: Logging.general_logs; }
            }
            Tab {
                title: "Proxy";
                LogView { logs: Logging.proxy_logs; }
            }
            // ... 其他标签页
        }
    }
}
7. 错误处理
7.1 错误类型
rust


#[derive(Debug)]
pub enum LogError {
    ChannelSendError(mpsc::error::SendError<LogEntry>),
    StorageError(String),
    UIUpdateError(String),
}
7.2 错误处理策略
使用 Result 类型处理所有可能的错误
实现错误转换和传播
提供错误恢复机制
8. 扩展性设计
8.1 添加新的日志目标
rust


// 注册新的日志目标
log_manager.register_target(LogTarget::Custom("NewModule".to_string()));
8.2 自定义处理器
rust


pub trait LogProcessor {
    fn process(&self, entry: LogEntry) -> Result<LogEntry, LogError>;
}

// 实现自定义处理器
pub struct CustomLogHandler {
    processor: Box<dyn LogProcessor>,
}
9. 配置选项
9.1 系统配置
rust


pub struct LogConfig {
    pub max_logs: usize,
    pub batch_size: usize,
    pub update_interval: Duration,
    pub enable_file_logging: bool,
    pub log_file_path: Option<String>,
}
9.2 默认配置
rust


impl Default for LogConfig {
    fn default() -> Self {
        Self {
            max_logs: 1000,
            batch_size: 100,
            update_interval: Duration::from_millis(100),
            enable_file_logging: false,
            log_file_path: None,
        }
    }
}
10. 最佳实践
10.1 使用建议
合理配置批处理大小和更新间隔
及时清理不需要的日志
使用适当的日志级别
10.2 性能注意事项
避免过频繁的UI更新
控制内存使用
合理使用异步操作
10.3 扩展开发
遵循现有的接口约定
实现必要的特征
保持代码结构一致性
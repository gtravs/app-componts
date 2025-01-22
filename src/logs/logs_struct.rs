use std::fmt::Debug;

use slint::SharedString;


// 多组件日志数据结构
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum LogTarget {
    EventLog,
    ProxyLog,
    Custom(String)
}

pub trait LogEntry: Default+Clone+Debug+Send+'static {
    fn get_timestamp(&self) -> String;
    fn get_level(&self) -> String;
    fn to_string_fmt(&self) -> String;
}


// 代理日志格式
// #[derive(Clone, Debug,Default)]
// pub struct ProxyLogEntry {
//     pub timestamp: SharedString,
//     pub level: SharedString,
//     pub id: SharedString,
//     pub host: SharedString,
//     pub method: SharedString,
//     pub url: SharedString,
//     pub status_code: SharedString,
//     pub length: SharedString,
// }

// impl LogEntry for ProxyLogEntry {
//     fn get_timestamp(&self) -> String {
//         (&self.timestamp).to_string()
//     }

//     fn get_level(&self) -> String {
//         (&self.level).to_string()
//     }

//     fn to_string_fmt(&self) -> String {
//         format!("[{}] {} - {} {} {} - Status: {} Length: {}",
//         self.timestamp, self.level, self.host,
//         self.method, self.url, self.status_code,
//         self.length
//     )
//     }
// }
// 通用日志格式
// #[derive(Clone, Debug,Default)]
// pub struct EventLog {
//     pub level: SharedString,
//     pub message: SharedString,
//     pub timestamp: SharedString,
// }

// impl LogEntry for EventLog {
//     fn get_timestamp(&self) -> String {
//         (&self.timestamp).to_string()
//     }

//     fn get_level(&self) -> String {
//         (&self.level).to_string()
//     }

//     fn to_string_fmt(&self) -> String {
//         format!("[{}] {} - {}", 
//         self.timestamp, self.level, self.message
//     )
//     }
// }




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
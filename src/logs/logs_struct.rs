use std::fmt::Debug;

use slint::SharedString;

use crate::slint_generatedApp;

use super::logs::LogEntry;



// 代理日志格式
#[derive(Clone, Debug,Default)]
pub struct ProxyLogEntry {
    pub timestamp: SharedString,
    pub id: SharedString,
    pub host: SharedString,
    pub method: SharedString,
    pub url: SharedString,
    pub status_code: SharedString,
    pub length: SharedString,
}
impl From<super::logs_struct::ProxyLogEntry> for slint_generatedApp::ProxyLogEntry {
    fn from(entry: super::logs_struct::ProxyLogEntry) -> Self {
        slint_generatedApp::ProxyLogEntry {
            timestamp: entry.timestamp.into(),
            id: entry.id.into(),
            host: entry.host.into(),
            method: entry.method.into(),
            url: entry.url.into(),
            status_code: entry.status_code.into(),
            length: entry.length.into()
        }
    }
}
impl LogEntry for ProxyLogEntry {}


// 通用日志格式
#[derive(Clone, Debug,Default)]
pub struct EventLog {
    pub level: SharedString,
    pub message: SharedString,
    pub timestamp: SharedString,
}
impl From<super::logs_struct::EventLog> for slint_generatedApp::EventLog {
    fn from(entry: super::logs_struct::EventLog) -> Self {
        slint_generatedApp::EventLog {
            level: entry.level.into(),
            message: entry.message.into(),
            timestamp: entry.timestamp.into(),
        }
    }
}
impl LogEntry for EventLog {}

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




// #[derive(Clone, Debug)]
// pub enum LogLevel {
//     Debug,
//     Info,
//     Warning,
//     Error,
// }

// impl LogLevel {
//     pub fn as_str(&self) -> &'static str {
//         match self {
//             LogLevel::Debug => "DEBUG",
//             LogLevel::Info => "INFO",
//             LogLevel::Warning => "WARN",
//             LogLevel::Error => "ERROR",
//         }
//     }
// }
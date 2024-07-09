//日志等级定义
#[derive(Debug, PartialEq, PartialOrd, Eq, Hash, Clone, Copy)]
pub enum LogLevel {
    Debug,
    Warning,
    Info,
    Error,
}

impl From<i32> for LogLevel {
    fn from(n: i32) -> Self {
        match n {
            1 => LogLevel::Debug,
            2 => LogLevel::Warning,
            3 => LogLevel::Info,
            4 => LogLevel::Error,
            _ => LogLevel::Debug,
        }
    }
}

impl Into<i32> for LogLevel {
    fn into(self) -> i32 {
        match self {
            LogLevel::Debug => 1,
            LogLevel::Warning => 2,
            LogLevel::Info => 3,
            LogLevel::Error => 4,
        }
    }
}

mod inner;
pub use inner::Inner;

mod outter;
pub use outter::Outter;

mod hub;
pub use hub::init;
mod sink;
pub use sink::clone_sender;

pub fn build_logger(log_name: &str) -> Outter {
    let log_level = sink::get_global_log_level();
    let sender = clone_sender().unwrap();
    Outter::new(log_name)
        .with_level(log_level)
        .with_sinker(sender)
}

//暴露给用户的 Outter log对象
//每个 Outter 对象都持有一个文件路径，以及对应的日志等级
use super::LogLevel;
use chrono::Local;
use std::sync::mpsc::Sender;

#[derive(Clone)]
pub struct Outter {
    log_path: String, //文件路径
    level: LogLevel,  //当前设置的可写入的日志等级
    sinker: Option<Sender<(String, String)>>,
}

impl Outter {
    pub fn new(log_name: &str) -> Self {
        let log_path = format!("log/{log_name}");
        Outter {
            log_path,
            level: LogLevel::Debug, //默认当前是最低日志等级
            sinker: None,           //默认处理日志文本的方法是打印到 stdout
        }
    }

    pub fn with_level(mut self, lvl: LogLevel) -> Self {
        self.level = lvl;
        self
    }

    pub fn with_sinker(mut self, sinker: Sender<(String, String)>) -> Self {
        self.sinker = Some(sinker);
        self
    }

    pub fn get_path(&self) -> &str {
        self.log_path.as_str()
    }

    pub fn set_level(&mut self, lvl: LogLevel) {
        self.level = lvl;
    }

    pub fn get_level(&self) -> LogLevel {
        self.level
    }

    pub fn can_log_debug(&self) -> bool {
        LogLevel::Debug >= self.level
    }

    pub fn can_log_warning(&self) -> bool {
        LogLevel::Warning >= self.level
    }

    pub fn can_log_info(&self) -> bool {
        LogLevel::Info >= self.level
    }

    pub fn can_log_error(&self) -> bool {
        LogLevel::Error >= self.level
    }

    pub fn log(&mut self, lvl: &str, logstr: &str) {
        let timestr = Local::now().format("%Y-%m-%d %H:%M:%S%.6f").to_string();
        let nstr = format!("[{}][{}]{}", timestr, lvl, logstr);
        let fp = self.get_path().to_string();
        if let Some(sinker) = &self.sinker {
            if let Err(err) = sinker.send((fp, nstr)) {
                eprintln!("[log]: err={err}, logstr={logstr}");
            }
        } else {
            eprintln!("[log]: no_sinker=true,logstr={logstr}");
        }
    }
}

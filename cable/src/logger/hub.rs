use super::sink;
use super::{inner::Inner, LogLevel};
use sink::LogMsgType;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;

//只能初始化一次
pub fn init(log_level: LogLevel, log_chan_size: usize) {
    if sink::is_init() {
        return;
    }
    sink::set_init();

    sink::set_global_log_level(log_level);
    sink::set_chan(log_chan_size);
    let chan_receiver = sink::take_receiver().unwrap();

    thread::spawn(move || {
        let mut logfiles: HashMap<String, Inner> = HashMap::new();
        loop {
            match chan_receiver.recv() {
                Ok((log_path, logstr)) => {
                    if logstr == "gm:close" {
                        eprintln!("gm:close");
                        break;
                    }
                    if let Some(lgr) = logfiles.get_mut(&log_path) {
                        if let Err(err) = lgr.write(&logstr) {
                            eprintln!("log failed: {err}");
                        }
                    } else {
                        let fp = PathBuf::from(log_path.clone());
                        let filename = fp.file_name().unwrap().to_str().unwrap();
                        let mut lgr = Inner::new(&log_path, filename);
                        if let Err(err) = lgr.write(&logstr) {
                            eprintln!("log failed: {err}");
                        }
                        logfiles.insert(log_path, lgr);
                    }
                }
                Err(err) => {
                    eprintln!("log failed: {err}");
                }
            }
        }
    });
}

pub fn clone_sender() -> Option<mpsc::Sender<LogMsgType>> {
    sink::clone_sender()
}

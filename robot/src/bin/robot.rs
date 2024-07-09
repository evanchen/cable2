use cable::config::Config;
use cable::logger::{init, LogLevel};
use robot::services;
use std::env;

fn main() {
    let pwd = env::current_dir().unwrap();
    let config_path = format!("{}/etc/sysconfig.conf", pwd.to_str().unwrap());
    let conf = Config::new(&config_path).with("workdir", pwd.to_str().unwrap());
    let log_level: LogLevel = conf.get_int("log_level").unwrap().into();
    let log_chan_size = conf.get_int("log_chan_size").unwrap() as usize;
    init(log_level, log_chan_size);
    services::start(conf);
}

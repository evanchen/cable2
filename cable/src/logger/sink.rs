use super::LogLevel;
use lazy_static::lazy_static;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{atomic, Mutex};

lazy_static! {
    static ref G_REMOTER: Mutex<Sink> = Mutex::new(Sink::new());
}

pub type LogMsgType = (String, String);

pub struct Sink {
    level: LogLevel,
    sender: Option<Sender<LogMsgType>>,
    receiver: Option<Receiver<LogMsgType>>,
    init: atomic::AtomicBool,
}

impl Sink {
    pub fn new() -> Self {
        Sink {
            level: LogLevel::Debug,
            sender: None,
            receiver: None,
            init: atomic::AtomicBool::new(false),
        }
    }

    pub fn set_level(&mut self, lvl: LogLevel) {
        self.level = lvl;
    }

    pub fn set_chan(&mut self, _log_chan_size: usize) {
        let (sender, receiver) = mpsc::channel();
        self.sender = Some(sender);
        self.receiver = Some(receiver);
    }

    pub fn get_level(&self) -> LogLevel {
        self.level
    }

    pub fn clone_sender(&self) -> Option<Sender<LogMsgType>> {
        self.sender.clone()
    }

    pub fn take_receiver(&mut self) -> Option<Receiver<LogMsgType>> {
        self.receiver.take()
    }

    pub fn is_init(&self) -> bool {
        self.init.load(atomic::Ordering::SeqCst)
    }

    pub fn set_init(&mut self) {
        self.init.store(true, atomic::Ordering::SeqCst)
    }
}

pub fn set_global_log_level(lvl: LogLevel) {
    let mut remote = G_REMOTER.lock().unwrap();
    (*remote).set_level(lvl);
}

pub fn get_global_log_level() -> LogLevel {
    let remote = G_REMOTER.lock().unwrap();
    (*remote).get_level()
}

pub fn set_chan(log_chan_size: usize) {
    let mut remote = G_REMOTER.lock().unwrap();
    (*remote).set_chan(log_chan_size);
}

pub fn clone_sender() -> Option<Sender<LogMsgType>> {
    let remote = G_REMOTER.lock().unwrap();
    (*remote).clone_sender()
}

pub fn take_receiver() -> Option<Receiver<LogMsgType>> {
    let mut remote = G_REMOTER.lock().unwrap();
    (*remote).take_receiver()
}

pub fn is_init() -> bool {
    let remote = G_REMOTER.lock().unwrap();
    (*remote).is_init()
}

pub fn set_init() {
    let mut remote = G_REMOTER.lock().unwrap();
    (*remote).set_init()
}

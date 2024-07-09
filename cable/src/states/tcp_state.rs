use super::Communicate;
use crate::message::SMSender;
use std::collections::HashMap;

pub struct TcpState {
    conn_map: HashMap<u64, SMSender>, //存放所有完成连接后，暴露给 tcp 服务的网路连接的消息 chan，映射 [vfd] = sender
}

impl TcpState {
    pub fn new() -> Self {
        TcpState {
            conn_map: HashMap::new(),
        }
    }
}

impl Communicate<SMSender> for TcpState {
    fn conn_map(&mut self) -> &mut HashMap<u64, SMSender> {
        &mut self.conn_map
    }
}

pub use crate::protos::ProtoType;

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum ServiceType {
    TCP,
    RPC,
    RPCCLIENT,
    TCPROBOT,
    DB,
    UNKNOW,
}

impl From<&str> for ServiceType {
    fn from(value: &str) -> Self {
        match value {
            "game_service" => Self::TCP,
            "rpc_service" => Self::RPC,
            "rpc_client_service" => Self::RPCCLIENT,
            "robot_service" => Self::TCPROBOT,
            "db_service" => Self::DB,
            _ => Self::UNKNOW,
        }
    }
}

impl Into<String> for ServiceType {
    fn into(self) -> String {
        match self {
            ServiceType::TCP => String::from("game_service"),
            ServiceType::RPC => String::from("rpc_service"),
            ServiceType::RPCCLIENT => String::from("rpc_client_service"),
            ServiceType::TCPROBOT => String::from("robot_service"),
            ServiceType::DB => String::from("db_service"),
            ServiceType::UNKNOW => String::from("unknow_service"),
        }
    }
}

// 组合消息类型，元组包含 (type,session,pto), session 是一个唯一标识,但可以回环重置
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum MessageType {
    Tcp,          //tcp监听套接字收到的消息
    Rpc,          //rpc监听套接字收到的消息
    RpcClient,    //rpc的客户端接收到的消息
    SocketClosed, //tcp连接断开
    Dummy,        //占位
}
pub type SystemMsg = (MessageType, u64, ProtoType);

use tokio::sync::mpsc::{Receiver, Sender};
//发送 SystemMsg 的 channel 类型
pub type SMSender = Sender<SystemMsg>;
//接收 SystemMsg 的 channel 类型
pub type SMReceiver = Receiver<SystemMsg>;

//发送 SMSender 的 channel 类型, (vfd,SMSender)
pub type SMSenderChan = Sender<(u64, SMSender)>;
//接收 SMSender 的 channel 类型, (vfd,SMSender)
pub type SMReceiverChan = Receiver<(u64, SMSender)>;

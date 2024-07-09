use std::fmt::{Display, Formatter, Result};
use tokio::sync::{mpsc, oneshot};

pub mod service;

#[derive(Debug)]
pub enum HttpProtoType {
    ReqServerAll,
    ReqServerInfo(u32),
    RespServerInfo(String),
    ReqGM(String),
    RespGM(String),
    Unimplemented(String),
}

impl Display for HttpProtoType {
    fn fmt(&self, f: &mut Formatter) -> Result {
        match self {
            HttpProtoType::ReqServerAll => {
                write!(f, "ReqServerAll")
            }
            HttpProtoType::ReqServerInfo(hostid) => {
                write!(f, "ReqServerInfo({})", hostid)
            }
            HttpProtoType::RespServerInfo(info) => {
                write!(f, "RespServerInfo({})", info)
            }
            HttpProtoType::ReqGM(cmdstr) => {
                write!(f, "ReqGM({})", cmdstr)
            }
            HttpProtoType::RespGM(cmdstr) => {
                write!(f, "RespGM({})", cmdstr)
            }
            HttpProtoType::Unimplemented(info) => {
                write!(f, "Unimplemented({})", info)
            }
        }
    }
}

pub type HttpProtoSenderOp = oneshot::Sender<HttpProtoType>;
pub type ProtoReceiverOp = oneshot::Receiver<HttpProtoType>;
pub type HttpProtoSenderMp = mpsc::Sender<HttpProtoType>;
pub type ProtoReceiverMp = mpsc::Receiver<HttpProtoType>;
pub type ChanHttpProtoSenderOp = mpsc::Sender<(HttpProtoType, HttpProtoSenderOp)>;
pub type ChanHttpProtoReceiverOp = mpsc::Receiver<(HttpProtoType, HttpProtoSenderOp)>;

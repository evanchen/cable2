use super::service::{ReadStreamMaybeTls, ReadStreamNoneTls, ReadStreamTls};
use crate::error::Error;
use crate::logger::{build_logger, Outter};
use crate::message::{MessageType, SMSender, SystemMsg};
use crate::network::tcp::{PROTO_BODY_MAX_LEN, PROTO_HEADER_LEN};
use crate::{debug, error, info, protos};
use futures_util::StreamExt;
use std::sync::Arc;
use tokio::sync::{
    broadcast,
    mpsc::{self, error::TrySendError},
    Semaphore,
};
use tokio_tungstenite::tungstenite::Message;

const LOG_NAME: &str = "ws_reader.log";

pub struct ConnReader {
    vfd: u64, //每个连接分配一个虚拟的唯一的fd
    stream_tls: Option<ReadStreamTls>,
    stream_ntls: Option<ReadStreamNoneTls>,
    stream_maybe_tls: Option<ReadStreamMaybeTls>,
    limit_connections: Arc<Semaphore>,
    readnum: u64,
    log: Outter,
    proto_sender: SMSender,
    _shutdown_complete: mpsc::Sender<()>, // 对象销毁时自动销毁
    service_notify: Option<broadcast::Receiver<()>>,
    _pairdrop_sender: mpsc::Sender<()>, // 对象销毁时自动销毁
}

impl Drop for ConnReader {
    fn drop(&mut self) {
        self.limit_connections.add_permits(1);
    }
}

impl ConnReader {
    pub fn new(
        vfd: u64,
        stream_tls: Option<ReadStreamTls>,
        stream_ntls: Option<ReadStreamNoneTls>,
        stream_maybe_tls: Option<ReadStreamMaybeTls>,
        limit_connections: Arc<Semaphore>,
        proto_sender: SMSender,
        _shutdown_complete: mpsc::Sender<()>,
        service_notify: broadcast::Receiver<()>,
        _pairdrop_sender: mpsc::Sender<()>,
    ) -> ConnReader {
        let log: Outter = build_logger(LOG_NAME);
        ConnReader {
            vfd,
            stream_tls,
            stream_ntls,
            stream_maybe_tls,
            limit_connections,
            readnum: 0,
            log,
            proto_sender,
            _shutdown_complete,
            service_notify: Some(service_notify),
            _pairdrop_sender,
        }
    }

    pub async fn run(&mut self) -> crate::Result<()> {
        let mut service_notify = self.service_notify.take().unwrap();
        loop {
            tokio::select! {
                res = self.read_frame() => {
                    match res {
                        Ok(pto_op) => {
                            match pto_op {
                                Some(pto)=> {
                                    // 注意, 如果这里使用 send 发送会产生阻塞,而对端的消息处理完毕后也可能会有消息返回也是通过 send.
                                    // 如果这边的 send 出现阻塞, 对端返回的 send 也同样出现阻塞, 这时候会导致两端的协程产生 deadlock.
                                    // 解决的办法有 1) send 一个 oneshot 或者 2) 用 try_send 代替 send;
                                    // 用 1) 的弊端是必须在 send 的一端等待消息返回. 而用 try_send 的弊端则是发送失败时,只能选择丢弃消息
                                    // 这里选择 2), 因为这样做更符合背压原理, 对于游戏玩家的请求, 处理不过来就丢弃这也是合理的;
                                    // 但对于 rpc 的服务类型,如果确保不了发送消息的成功就会出现麻烦事.
                                    // :TODO: 对于 rpc 的发送, 后续是通过 spawn 一个协程来发送呢,还是有其他更好的办法.
                                    // 这里暂时的做法是把未发送成功的协议记录下来,通过日志的错误提示,再寻求扩大队列还是其他更好的办法.

                                    //self.proto_sender.send(pto).await?; // would block
                                    if let Err(err) = self.proto_sender.try_send(pto) {
                                        match err {
                                            TrySendError::Full(err) => {
                                                error!(self.log,"[ConnReader]: send=failed, msgtype={:?},vfd={}",err.0,err.1);
                                            },
                                            TrySendError::Closed(_err) =>{
                                                error!(self.log,"[ConnReader]: proto_sender=close, vfd={}",self.vfd);
                                                break;
                                            }
                                        }
                                    }
                                },
                                None => {
                                    //do nothing..
                                }
                            }
                        },
                        Err(err) => {
                            error!(self.log,"[ConnReader]: vfd={},err={}",self.vfd,err);
                            break;
                        }
                    }
                }
                _ = service_notify.recv() => {
                    info!(self.log,"[ConnReader]: notify_close=true,vfd={}",self.vfd);
                    break;
                },
            };
        }
        Ok(())
    }

    fn extract_msg(&mut self, res: Message) -> crate::Result<Option<SystemMsg>> {
        let buff = match res {
            Message::Binary(bin) => {
                //println!("[extract_msg]: read bin");
                bin
            }
            Message::Ping(_) => {
                println!("[extract_msg]: read ping");
                return Ok(None);
            }
            Message::Pong(_) => {
                println!("[extract_msg]: read ping");
                return Ok(None);
            }
            Message::Text(txt) => {
                println!("[extract_msg]: read text: {txt}");
                return Ok(None);
            }
            Message::Close(_) => {
                println!("[extract_msg]: read close");
                return Err(Error::Message("close".to_string()));
            }
            Message::Frame(_) => {
                println!("[extract_msg]: read frame");
                return Ok(None);
            }
        };
        if buff.len() < PROTO_HEADER_LEN {
            return Err(Error::Message("wrong header".to_string()));
        }
        let buff_body_len = buff.len() - PROTO_HEADER_LEN;
        if buff_body_len >= PROTO_BODY_MAX_LEN {
            return Err(format!(
                "[extract_msg]: buff_exceed=PROTO_BODY_MAX_LEN,{}",
                buff_body_len
            )
            .into());
        }
        let (header, body) = buff.split_at(PROTO_HEADER_LEN);
        //读消息头
        let mut proto_id = 0u32;
        proto_id |= header[0] as u32 & 0xff;
        proto_id |= (header[1] as u32 & 0xff) << 8;
        proto_id |= (header[2] as u32 & 0xff) << 16;
        proto_id |= (header[3] as u32 & 0xff) << 24;

        let mut body_len = 0u32;
        body_len |= header[4] as u32 & 0xff;
        body_len |= (header[5] as u32 & 0xff) << 8;
        body_len |= (header[6] as u32 & 0xff) << 16;
        body_len |= (header[7] as u32 & 0xff) << 24;

        debug!(
            self.log,
            "[extract_msg]: proto_id={proto_id},body_len={body_len},body={:?}", body
        );

        //协议长度超出最大上限
        if body_len != buff_body_len as u32 {
            return Err(format!(
                "[extract_msg]: body_len!=buff_body_len,{body_len},{buff_body_len}",
            )
            .into());
        }

        //读消息体
        //解码
        match protos::decode(proto_id, body) {
            Ok(ptoobj) => {
                self.readnum += 1;
                debug!(
                    self.log,
                    "[extract_msg]: proto_id={},buflen={},readnum={}",
                    proto_id,
                    body_len,
                    self.readnum,
                );
                Ok(Some((MessageType::Tcp, self.vfd, ptoobj)))
            }
            Err(err) => Err(err.into()),
        }
    }

    pub async fn read_frame(&mut self) -> crate::Result<Option<SystemMsg>> {
        if self.stream_tls.is_some() {
            let stream = self.stream_tls.as_mut().unwrap();
            if let Some(res) = stream.next().await {
                match res {
                    Ok(msg) => self.extract_msg(msg),
                    Err(err) => {
                        return Err(Error::Message(err.to_string()));
                    }
                }
            } else {
                return Err(Error::Message("next() empty".to_string()));
            }
        } else if self.stream_ntls.is_some() {
            let stream = self.stream_ntls.as_mut().unwrap();
            if let Some(res) = stream.next().await {
                match res {
                    Ok(msg) => self.extract_msg(msg),
                    Err(err) => {
                        return Err(Error::Message(err.to_string()));
                    }
                }
            } else {
                return Err(Error::Message("next() empty".to_string()));
            }
        } else {
            let stream = self.stream_maybe_tls.as_mut().unwrap();
            if let Some(res) = stream.next().await {
                match res {
                    Ok(msg) => self.extract_msg(msg),
                    Err(err) => {
                        return Err(Error::Message(err.to_string()));
                    }
                }
            } else {
                return Err(Error::Message("next() empty".to_string()));
            }
        }
    }
}

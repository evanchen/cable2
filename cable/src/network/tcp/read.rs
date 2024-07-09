use super::{PROTO_BODY_MAX_LEN, PROTO_HEADER_LEN};
use crate::logger::{build_logger, Outter};
use crate::message::{MessageType, SMSender, ServiceType, SystemMsg};
use crate::{debug, error, info, protos};
use bytes::BytesMut;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, BufReader};
use tokio::net::tcp::OwnedReadHalf;
use tokio::sync::{
    broadcast,
    mpsc::{self, error::TrySendError},
    Semaphore,
};

const LOG_NAME: &str = "tcp_reader.log";

pub struct ConnReader {
    service_type: ServiceType,
    vfd: u64, //每个连接分配一个虚拟的唯一的fd
    stream: BufReader<OwnedReadHalf>,
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
        service_type: ServiceType,
        vfd: u64,
        stream: OwnedReadHalf,
        limit_connections: Arc<Semaphore>,
        proto_sender: SMSender,
        _shutdown_complete: mpsc::Sender<()>,
        service_notify: broadcast::Receiver<()>,
        _pairdrop_sender: mpsc::Sender<()>,
    ) -> ConnReader {
        let log: Outter = build_logger(LOG_NAME);
        ConnReader {
            service_type,
            vfd,
            stream: BufReader::new(stream),
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
                        Ok(pto) => {
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
                        Err(err) => {
                            info!(self.log,"[ConnReader]: closed=true,vfd={},err={}",self.vfd,err);
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

    pub async fn read_frame(&mut self) -> crate::Result<SystemMsg> {
        //读消息头
        let mut header = vec![0; PROTO_HEADER_LEN];
        let _rsize = self.stream.read_exact(&mut header).await?;
        //println!("rsize1 = {}", _rsize);
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

        //协议长度超出最大上限
        if body_len >= PROTO_BODY_MAX_LEN as u32 {
            return Err(format!("[parse_fram]: exceed=PROTO_BODY_MAX_LEN,{}", body_len).into());
        }

        //println!("[read_frame]: body_len={},proto_id={}", body_len, proto_id);
        //读消息体
        let mut body = vec![0; body_len as usize];
        let _rsize = self.stream.read_exact(&mut body).await?;
        //println!("rsize2 = {}", _rsize);

        //解码
        match protos::decode(proto_id, &body) {
            Ok(ptoobj) => {
                self.readnum += 1;
                debug!(
                    self.log,
                    "[read_frame]: proto_id={},buflen={},readnum={}",
                    proto_id,
                    body_len,
                    self.readnum,
                );
                let msg_type = match self.service_type {
                    ServiceType::TCP => MessageType::Tcp,
                    ServiceType::RPC => MessageType::Rpc,
                    ServiceType::RPCCLIENT => MessageType::RpcClient,
                    _ => MessageType::Dummy,
                };
                Ok((msg_type, self.vfd, ptoobj))
            }
            Err(err) => Err(err.into()),
        }
    }
}

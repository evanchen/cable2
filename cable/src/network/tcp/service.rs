use crate::logger::Outter;
use crate::protos::{Dummy, ProtoType};
use crate::{config::Config, error::Error};
use crate::{error, info};
use std::sync::Arc;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{broadcast, mpsc, Semaphore},
    time::{self, Duration},
};

use super::{read::ConnReader, write::ConnWriter};
use crate::message::{MessageType, SMSender, SMSenderChan, ServiceType};

pub struct Service {
    pub service_type: ServiceType,
    pub service_addr: String,
    pub listener: Option<TcpListener>,
    pub limit_connections: Arc<Semaphore>,
    pub counter: u64,
    pub log: Outter,
    pub conf: Config,
    pub notify_client_shutdown: broadcast::Sender<()>,
    pub shutdown_complete_sender: mpsc::Sender<()>,
    pub shutdown_complete_receiver: mpsc::Receiver<()>,
    pub chan_sender: SMSenderChan, //vfd 暴露出来的私有 chan 传递给外面，外面有信息传入给对应的 vfd 时，通过这个 chan 传入
    pub msg_sender: SMSender,      //vfd 从网络读取消息时发送到外面处理
}

pub fn build(
    service_type: ServiceType,
    conf: Config,
    log: Outter,
    addr: String,
    msg_sender: SMSender,
    chan_sender: SMSenderChan,
) -> Service {
    Service::new(service_type, conf, log, addr, msg_sender, chan_sender)
}

impl Service {
    pub fn new(
        service_type: ServiceType,
        conf: Config,
        log: Outter,
        addr: String,
        msg_sender: SMSender,
        chan_sender: SMSenderChan,
    ) -> Self {
        let (notify_client_shutdown, _) = broadcast::channel(1);
        let (shutdown_complete_sender, shutdown_complete_receiver) = mpsc::channel(1);

        let max_connection = conf.get_int("max_connection").unwrap() as usize;
        Service {
            service_type,
            service_addr: addr,
            listener: None,
            limit_connections: Arc::new(Semaphore::new(max_connection)),
            counter: 100, //正常的vfd从100开始, 有些vfd预留特殊使用,比如 vfd=1, 是广播
            log,
            conf,
            notify_client_shutdown,
            shutdown_complete_sender,
            shutdown_complete_receiver,
            chan_sender,
            msg_sender,
        }
    }

    pub fn inc_counter(&mut self) -> u64 {
        self.counter += 1;
        self.counter
    }

    pub async fn init_listener(&mut self) -> crate::Result<()> {
        assert!(self.listener.is_none());
        self.listener = Some(TcpListener::bind(&self.service_addr).await.unwrap());
        Ok(())
    }

    pub async fn run(mut self) -> crate::Result<()> {
        self.init_listener().await?;
        self.start_loop().await?;

        let Service {
            mut shutdown_complete_receiver,
            shutdown_complete_sender,
            notify_client_shutdown,
            ..
        } = self;

        drop(notify_client_shutdown);
        drop(shutdown_complete_sender);
        shutdown_complete_receiver.recv().await;

        Ok(())
    }

    async fn start_loop(&mut self) -> crate::Result<()> {
        loop {
            let stream = self.accept().await?;
            let vfd = self.inc_counter();
            self.handle_stream(stream, vfd).await?;
        }
    }

    pub async fn accept(&mut self) -> crate::Result<TcpStream> {
        let mut backoff = 1;
        loop {
            match self.listener.as_mut().unwrap().accept().await {
                Ok((socket, _)) => return Ok(socket),
                Err(err) => {
                    if backoff > 64 {
                        return Err(err.into());
                    }
                }
            }
            time::sleep(Duration::from_secs(backoff)).await;
            backoff *= 2;
            info!(self.log, "[accept]: backoff={}", backoff);
        }
    }

    pub fn split_stream(
        &mut self,
        stream: TcpStream,
        identify: u64,
    ) -> (ConnReader, ConnWriter, SMSender) {
        let vfd = identify;
        let (pairdrop_sender, pairdrop_receiver) = mpsc::channel(1);
        let (read_stream, write_stream) = stream.into_split();
        let reader = ConnReader::new(
            self.service_type,
            vfd,
            read_stream,
            self.limit_connections.clone(),
            self.msg_sender.clone(),
            self.shutdown_complete_sender.clone(),
            self.notify_client_shutdown.subscribe(),
            pairdrop_sender,
        );

        // 根据服务类型决定 channel 队列大小
        let conn_msg_chan_size = self.conf.get_int("conn_msg_chan_size").unwrap() as usize;
        let (conn_tx, conn_rx) = mpsc::channel(conn_msg_chan_size);
        let writer = ConnWriter::new(
            self.service_type,
            vfd,
            write_stream,
            conn_rx,
            pairdrop_receiver,
        );

        (reader, writer, conn_tx)
    }

    pub async fn handle_stream(&mut self, stream: TcpStream, identify: u64) -> crate::Result<()> {
        // 给下一个新连接分配一个自增的唯一id
        let vfd = identify;
        let (reader, writer, conn_tx) = self.split_stream(stream, vfd);
        // 在 reader 被 drop 时归还计数
        self.limit_connections.acquire().await.unwrap().forget();

        // 暴露自己的消息输入端给外界, :TODO: 注意这里会产生阻塞
        if let Err(_err) = self.chan_sender.send((vfd, conn_tx)).await {
            let errstr = format!("[run]: vfd={vfd},chan_sender=err");
            return Err(Error::Message(errstr));
        }
        self.start_read_write(vfd, reader, writer).await
    }

    async fn start_read_write(
        &mut self,
        vfd: u64,
        mut reader: ConnReader,
        mut writer: ConnWriter,
    ) -> crate::Result<()> {
        let mut wlog = self.log.clone();
        // 开启 socket 消息写循环
        tokio::spawn(async move {
            if let Err(err) = writer.run().await {
                error!(wlog, "[ConnWriter]: error: vfd={},{:?}", vfd, err);
            } else {
                info!(wlog, "[ConnWriter]: return,vfd={}", vfd);
            }
        });

        // 开启socket 消息读循环
        let close_notify = self.msg_sender.clone();
        let mut rlog = self.log.clone();
        tokio::spawn(async move {
            if let Err(err) = reader.run().await {
                error!(rlog, "[ConnReader]: error: vfd={},{:?}", vfd, err);
            } else {
                info!(rlog, "[ConnReader]: return,vfd={}", vfd);
            }
            let dummy = Dummy::default();
            let _ = close_notify
                .send((MessageType::SocketClosed, vfd, ProtoType::Dummy(dummy)))
                .await;
        });
        Ok(())
    }
}

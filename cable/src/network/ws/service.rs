use super::{read::ConnReader, write::ConnWriter};
use crate::error::Error;
use crate::message::{MessageType, ProtoType};
use crate::network::tcp::service::Service;
use crate::protos::Dummy;
use crate::{error, info};
use futures_util::stream::SplitSink;
use futures_util::stream::SplitStream;
use futures_util::stream::StreamExt;
use native_tls::Identity;
use std::{fs::File, io::Read};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_native_tls::TlsStream;
use tokio_native_tls::{native_tls, TlsAcceptor};
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::WebSocketStream;

pub type ReadStreamTls = SplitStream<WebSocketStream<TlsStream<TcpStream>>>;
pub type ReadStreamNoneTls = SplitStream<WebSocketStream<TcpStream>>;

pub type WriteStreamTls = SplitSink<WebSocketStream<TlsStream<TcpStream>>, Message>;
pub type WriteStreamNoneTls = SplitSink<WebSocketStream<TcpStream>, Message>;

pub type ReadStreamMaybeTls = SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>;
pub type WriteStreamMaybeTls = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;

impl Service {
    pub async fn run_as_websocket(mut self) -> crate::Result<()> {
        self.init_listener().await?;
        let is_ssl = self.conf.get_bool("is_ssl");
        let acceptor = if is_ssl {
            // Load SSL/TLS certificate and private key
            let cert_path = self.conf.get_string("certificate_file").unwrap();
            let key_path = self.conf.get_string("privatekey_file").unwrap();

            let mut cert_file = File::open(cert_path).expect("Failed to open certificate file");
            let mut key_file = File::open(key_path).expect("Failed to open private key file");

            let mut cert_buffer = Vec::new();
            let mut key_buffer = Vec::new();

            cert_file
                .read_to_end(&mut cert_buffer)
                .expect("Failed to read certificate file");
            key_file
                .read_to_end(&mut key_buffer)
                .expect("Failed to read private key file");

            let identity = Identity::from_pkcs8(&cert_buffer, &key_buffer).unwrap();
            let acceptor = native_tls::TlsAcceptor::new(identity).unwrap();
            let acceptor = tokio_native_tls::TlsAcceptor::from(acceptor);
            Some(acceptor)
        } else {
            None
        };

        self.start_loop_as_websocket(acceptor).await?;

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

    async fn start_loop_as_websocket(
        &mut self,
        acceptor: Option<TlsAcceptor>,
    ) -> crate::Result<()> {
        if acceptor.is_some() {
            let a1 = acceptor.unwrap();
            loop {
                let stream = self.accept().await?;
                let a2 = a1.clone();
                // Wrap the TCP stream with SSL/TLS
                match a2.accept(stream).await {
                    Ok(stream_tls) => {
                        // Accept the WebSocket handshake
                        self.split(Some(stream_tls), None).await?;
                    }
                    Err(err) => {
                        error!(
                            self.log,
                            "[start_loop_as_websocket]: TlsAcceptor=true,err={err}"
                        );
                    }
                }
            }
        } else {
            loop {
                let stream_ntls = self.accept().await?;
                // Accept the WebSocket handshake
                self.split(None, Some(stream_ntls)).await?;
            }
        }
    }

    async fn split(
        &mut self,
        stream_tls: Option<TlsStream<TcpStream>>,
        stream_ntls: Option<TcpStream>,
    ) -> crate::Result<()> {
        if stream_tls.is_some() {
            let stream = stream_tls.unwrap();
            match accept_async(stream).await {
                Ok(ws_stream) => {
                    self.handle_stream_tls(ws_stream).await?;
                }
                Err(err) => {
                    error!(
                        self.log,
                        "[start_loop_as_websocket]: accept_async=true,err={err}"
                    );
                }
            }
        } else {
            let stream = stream_ntls.unwrap();
            match accept_async(stream).await {
                Ok(ws_stream) => {
                    self.handle_stream_none_tls(ws_stream).await?;
                }
                Err(err) => {
                    error!(
                        self.log,
                        "[start_loop_as_websocket]: accept_async=true,err={err}"
                    );
                }
            }
        }
        Ok(())
    }

    pub async fn handle_stream_tls(
        &mut self,
        stream: WebSocketStream<TlsStream<TcpStream>>,
    ) -> crate::Result<()> {
        // 给下一个新连接分配一个自增的唯一id
        let vfd = self.inc_counter();

        let (pairdrop_sender, pairdrop_receiver) = mpsc::channel(1);
        let (write_stream, read_stream) = stream.split();
        let reader = ConnReader::new(
            vfd,
            Some(read_stream),
            None,
            None,
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
            vfd,
            Some(write_stream),
            None,
            None,
            conn_rx,
            pairdrop_receiver,
        );

        // 在 reader 被 drop 时归还计数
        self.limit_connections.acquire().await.unwrap().forget();

        // 暴露自己的消息输入端给外界, :TODO: 注意这里会产生阻塞
        if let Err(_err) = self.chan_sender.send((vfd, conn_tx)).await {
            let errstr = format!("[run]: vfd={vfd},chan_sender=err");
            return Err(Error::Message(errstr));
        }

        self.start_read_write_ws(vfd, reader, writer).await
    }

    pub async fn handle_stream_none_tls(
        &mut self,
        stream: WebSocketStream<TcpStream>,
    ) -> crate::Result<()> {
        // 给下一个新连接分配一个自增的唯一id
        let vfd = self.inc_counter();

        let (pairdrop_sender, pairdrop_receiver) = mpsc::channel(1);
        let (write_stream, read_stream) = stream.split();
        let reader = ConnReader::new(
            vfd,
            None,
            Some(read_stream),
            None,
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
            vfd,
            None,
            Some(write_stream),
            None,
            conn_rx,
            pairdrop_receiver,
        );

        // 在 reader 被 drop 时归还计数
        self.limit_connections.acquire().await.unwrap().forget();

        // 暴露自己的消息输入端给外界, :TODO: 注意这里会产生阻塞
        if let Err(_err) = self.chan_sender.send((vfd, conn_tx)).await {
            let errstr = format!("[run]: vfd={vfd},chan_sender=err");
            return Err(Error::Message(errstr));
        }

        self.start_read_write_ws(vfd, reader, writer).await
    }

    async fn start_read_write_ws(
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

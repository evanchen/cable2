// 这个模块主要是用来辅助测试
use crate::error::Error;
use crate::network::tcp::client_service::Client;
use crate::{error, info};
use futures_util::StreamExt;
use tokio::sync::mpsc;
use tokio_tungstenite::connect_async;

use super::{read::ConnReader, write::ConnWriter};

impl Client {
    pub async fn run_as_websocket(mut self, addr: &str) -> crate::Result<()> {
        let is_ssl = self.conf.get_bool("is_ssl");
        let wsaddr = if is_ssl {
            format!("wss://{addr}/ws/")
        } else {
            format!("ws://{addr}/ws/")
        };
        info!(self.log, "[run]: connecting to {wsaddr}...");
        let (ws_stream, _) = connect_async(wsaddr).await.expect("ws failed to connect");
        println!("WebSocket handshake has been successfully completed");

        let (write_stream, read_stream) = ws_stream.split();
        let (pairdrop_sender, pairdrop_receiver) = mpsc::channel(1);

        let vfd = self.inc_counter();
        let mut reader = ConnReader::new(
            vfd,
            None,
            None,
            Some(read_stream),
            self.limit_connections.clone(),
            self.msg_sender.clone(),
            self.shutdown_complete_sender.clone(),
            self.notify_client_shutdown.subscribe(),
            pairdrop_sender,
        );

        // 根据服务类型决定 channel 队列大小
        let conn_msg_chan_size = self.conf.get_int("conn_msg_chan_size").unwrap() as usize;
        let (conn_tx, conn_rx) = mpsc::channel(conn_msg_chan_size);
        let mut writer = ConnWriter::new(
            vfd,
            None,
            None,
            Some(write_stream),
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

        // 开启 socket 消息写循环
        let mut wlog = self.log.clone();
        tokio::spawn(async move {
            if let Err(err) = writer.run().await {
                error!(wlog, "[ConnWriter]: error: vfd={},{:?}", vfd, err);
            } else {
                info!(wlog, "[ConnWriter]: return,vfd={}", vfd);
            }
        });

        // 开启socket 消息读循环
        let close_notify = self.conn_close_sender.clone();
        let mut rlog = self.log.clone();
        tokio::spawn(async move {
            if let Err(err) = reader.run().await {
                error!(rlog, "[ConnReader]: error: vfd={},{:?}", vfd, err);
            } else {
                info!(rlog, "[ConnReader]: return,vfd={}", vfd);
            }
            let _ = close_notify.send(vfd).await;
        });

        let Client {
            mut shutdown_complete_receiver,
            shutdown_complete_sender,
            notify_client_shutdown,
            ..
        } = self;

        shutdown_complete_receiver.recv().await;

        drop(notify_client_shutdown);
        drop(shutdown_complete_sender);

        Ok(())
    }
}

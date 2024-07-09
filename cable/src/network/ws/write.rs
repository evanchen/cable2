use super::service::{WriteStreamMaybeTls, WriteStreamNoneTls, WriteStreamTls};
use crate::error::Error;
use crate::logger::{build_logger, Outter};
use crate::message::SMReceiver;
use crate::network::tcp::PROTO_HEADER_LEN;
use crate::{debug, error, info, protos};
use futures_util::SinkExt;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

const LOG_NAME: &str = "ws_writer.log";

pub struct ConnWriter {
    vfd: u64,
    stream_tls: Option<WriteStreamTls>,
    stream_ntls: Option<WriteStreamNoneTls>,
    stream_maybe_tls: Option<WriteStreamMaybeTls>,
    writenum: u64,
    log: Outter,
    msg_receiver: SMReceiver,
    pairdrop_receiver: mpsc::Receiver<()>,
}

impl ConnWriter {
    pub fn new(
        vfd: u64,
        stream_tls: Option<WriteStreamTls>,
        stream_ntls: Option<WriteStreamNoneTls>,
        stream_maybe_tls: Option<WriteStreamMaybeTls>,
        msg_receiver: SMReceiver,
        pairdrop_receiver: mpsc::Receiver<()>,
    ) -> ConnWriter {
        let log = build_logger(LOG_NAME);
        ConnWriter {
            vfd,
            stream_tls,
            stream_ntls,
            stream_maybe_tls,
            writenum: 0,
            log,
            msg_receiver,
            pairdrop_receiver,
        }
    }

    pub async fn run(&mut self) -> crate::Result<()> {
        loop {
            tokio::select! {
                res = self.msg_receiver.recv() => {
                    if let Some((msg_type, from_vfd, pto)) = res {
                        if from_vfd >= 100  && self.vfd != from_vfd {
                            info!(
                                self.log,
                                "[ConnWriter]: wrong=true, vfd={}, from_vfd={}", self.vfd, from_vfd
                            );
                        }
                        let (proto_id,_) = pto.inner_info();
                        let buf = protos::encode(pto)?;
                        if let Err(err) = self.write_frame(proto_id, &buf).await {
                            error!(
                                self.log,
                                "[ConnWriter]: closed=true,vfd={},proto_id={},err={}", self.vfd, proto_id, err
                            );
                            break;
                        }
                    }
                },
                _ = self.pairdrop_receiver.recv() => {
                    info!(
                        self.log,
                        "[ConnWriter]: readhalf=drop, vfd={}", self.vfd,
                    );
                    break;
                }
            }
        }
        Ok(())
    }

    pub async fn write_frame(&mut self, proto_id: u32, buf: &[u8]) -> crate::Result<()> {
        self.writenum += 1;

        let buflen = buf.len() as u32;
        // little-endian
        // let mut header = 0u64;
        // header |= proto_id as u64;
        // header |= ((buflen as u64) & 0xffffffff) << 32;
        let whole_len = PROTO_HEADER_LEN + buflen as usize;
        let mut whole_buff = Vec::with_capacity(whole_len);
        whole_buff.push((proto_id & 0xff) as u8);
        whole_buff.push(((proto_id >> 8) & 0xff) as u8);
        whole_buff.push(((proto_id >> 16) & 0xff) as u8);
        whole_buff.push(((proto_id >> 24) & 0xff) as u8);

        whole_buff.push((buflen & 0xff) as u8);
        whole_buff.push(((buflen >> 8) & 0xff) as u8);
        whole_buff.push(((buflen >> 16) & 0xff) as u8);
        whole_buff.push(((buflen >> 24) & 0xff) as u8);

        whole_buff.extend_from_slice(buf);
        debug!(
            self.log,
            "[write_frame]: proto_id={},buflen={},writenum={},body={:?}",
            proto_id,
            buflen,
            self.writenum,
            buf
        );
        if self.stream_tls.is_some() {
            let stream = self.stream_tls.as_mut().unwrap();
            if let Err(err) = stream.send(Message::binary(whole_buff)).await {
                return Err(Error::Message(err.to_string()));
            }
        } else if self.stream_maybe_tls.is_none() {
            let stream = self.stream_ntls.as_mut().unwrap();
            if let Err(err) = stream.feed(Message::binary(whole_buff)).await {
                return Err(Error::Message(err.to_string()));
            }
        } else {
            let stream = self.stream_maybe_tls.as_mut().unwrap();
            if let Err(err) = stream.feed(Message::binary(whole_buff)).await {
                return Err(Error::Message(err.to_string()));
            }
        }

        Ok(())
    }
}

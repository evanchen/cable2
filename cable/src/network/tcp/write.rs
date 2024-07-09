use crate::logger::{build_logger, Outter};
use crate::message::{MessageType, SMReceiver, ServiceType};
use crate::{debug, error, info, protos};
use std::io;
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::sync::mpsc;

const LOG_NAME: &str = "tcp_writer.log";

pub struct ConnWriter {
    service_type: ServiceType,
    vfd: u64,
    stream: BufWriter<OwnedWriteHalf>,
    writenum: u64,
    log: Outter,
    msg_receiver: SMReceiver,
    pairdrop_receiver: mpsc::Receiver<()>,
}

impl ConnWriter {
    pub fn new(
        service_type: ServiceType,
        vfd: u64,
        stream: OwnedWriteHalf,
        msg_receiver: SMReceiver,
        pairdrop_receiver: mpsc::Receiver<()>,
    ) -> ConnWriter {
        let log = build_logger(LOG_NAME);
        ConnWriter {
            service_type,
            vfd,
            stream: BufWriter::new(stream),
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
                        let mut proceed = false;
                        if self.vfd != from_vfd {
                            info!(
                                self.log,
                                "[ConnWriter]: wrong_vfd=true, vfd={}, from_vfd={}", self.vfd, from_vfd
                            );
                        }
                        let (proto_id,_) = pto.inner_info();
                        match self.service_type {
                            ServiceType::TCP => {
                                if msg_type != MessageType::Tcp {
                                    info!(
                                        self.log,
                                        "[ConnWriter]: wrong_msgType=true, service_type={:?},msg_type={:?} from_vfd={}", self.service_type,msg_type,from_vfd
                                    );
                                } else {
                                    proceed = true;
                                }
                            },
                            ServiceType::RPC | ServiceType::RPCCLIENT => {
                                if msg_type == MessageType::Rpc || msg_type == MessageType::RpcClient {
                                    proceed = true;
                                } else {
                                    info!(
                                        self.log,
                                        "[ConnWriter]: wrong_msgType=true, service_type={:?},msg_type={:?} from_vfd={}", self.service_type,msg_type,from_vfd
                                    );
                                }
                            },
                            _ => {
                                info!(
                                    self.log,
                                    "[ConnWriter]: unknow_serviceType=true, from_vfd={}", from_vfd
                                );
                            }
                        }
                        if proceed {
                            let buf = protos::encode(pto)?;
                            if let Err(err) = self.write_frame(proto_id, &buf).await {
                                error!(
                                    self.log,
                                    "[ConnWriter]: closed=true,vfd={},proto_id={},err={}", self.vfd, proto_id, err
                                );
                                break;
                            }
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

    pub async fn write_frame(&mut self, proto_id: u32, buf: &[u8]) -> io::Result<()> {
        self.writenum += 1;

        let buflen = buf.len() as u32;
        // little-endian
        let mut header = 0u64;
        header |= proto_id as u64;
        header |= ((buflen as u64) & 0xffffffff) << 32;

        // let mut header2 = [0u8;PROTO_HEADER_LEN];
        // header2[0] = (proto_id & 0xff) as u8;
        // header2[1] = ((proto_id >> 8) & 0xff) as u8;
        // header2[2] = ((proto_id >> 16) & 0xff) as u8;
        // header2[3] = ((proto_id >> 24) & 0xff) as u8;

        // header2[4] = (buflen & 0xff) as u8;
        // header2[5] = ((buflen >> 8) & 0xff) as u8;
        // header2[6] = ((buflen >> 16) & 0xff) as u8;
        // header2[7] = ((buflen >> 24) & 0xff) as u8;
        debug!(
            self.log,
            "[write_frame]: proto_id={},buflen={},writenum={}", proto_id, buflen, self.writenum,
        );
        //self.stream.write(&header2).await?;

        self.stream.write_u64_le(header).await?;
        self.stream.write_all(&buf).await?;

        // Ensure the encoded frame is written to the socket. The calls above
        // are to the buffered stream and writes. Calling `flush` writes the
        // remaining contents of the buffer to the socket.
        self.stream.flush().await
    }
}

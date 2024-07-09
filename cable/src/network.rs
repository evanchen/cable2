pub mod http;
pub mod tcp;
pub mod udp;
pub mod ws;

use crate::error::Error;
use crate::message::*;
use tokio::sync::mpsc::error::TrySendError;

// try_send 不会阻塞
pub fn try_send(sender: &SMSender, vfd: u64, pto: ProtoType) -> crate::Result<()> {
    inner_try_send(sender, MessageType::Tcp, vfd, pto)
}

pub fn try_send_rpc(sender: &SMSender, vfd: u64, pto: ProtoType) -> crate::Result<()> {
    inner_try_send(sender, MessageType::Rpc, vfd, pto)
}

fn inner_try_send(
    sender: &SMSender,
    msg_type: MessageType,
    vfd: u64,
    pto: ProtoType,
) -> crate::Result<()> {
    let (proto_id, name) = pto.inner_info();
    if let Err(err) = sender.try_send((msg_type, vfd, pto)) {
        match err {
            TrySendError::Full(_err) => {
                let res =
                    format!("[try_send]: send=chan_full,vfd={vfd},proto_id={proto_id},name={name}");
                return Err(Error::Message(res));
            }
            TrySendError::Closed(_err) => {
                let res = format!(
                    "[try_send]: send=chan_closed,vfd={vfd},proto_id={proto_id},name={name}"
                );
                return Err(Error::Message(res));
            }
        }
    }
    Ok(())
}

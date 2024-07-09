use crate::config::Config;
use crate::logger::build_logger;
use crate::message::{SMSender, SMSenderChan, ServiceType};
use crate::network::tcp::service as tcp_service;
use crate::{error, info};
use tokio::sync::mpsc;

pub fn start(
    service_type: ServiceType,
    conf: Config,
    is_ws: bool,
    srv_addr: String,
    log_name: &str,
    msg_sender: SMSender,
    chan_sender: SMSenderChan,
    all_srv_close_sender: mpsc::Sender<()>,
) {
    let mut log = build_logger(log_name);
    let tcpservice = tcp_service::build(
        service_type,
        conf,
        log.clone(),
        srv_addr,
        msg_sender,
        chan_sender,
    );

    // tcp service for connection
    tokio::spawn(async move {
        info!(
            log,
            "[tcp_hub]: service=start,service_type:={:?}", service_type
        );
        if is_ws {
            if let Err(err) = tcpservice.run_as_websocket().await {
                error!(log, "[tcp_hub]: err={:?}", err);
            }
        } else {
            if let Err(err) = tcpservice.run().await {
                error!(log, "[tcp_hub]: err={:?}", err);
            }
        }
        drop(all_srv_close_sender);
        info!(log, "[tcp_hub]: service=stop");
    });
}

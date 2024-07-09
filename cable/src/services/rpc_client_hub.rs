use std::collections::HashMap;

use crate::config::Config;
use crate::logger::build_logger;
use crate::message::{MessageType, ProtoType, ServiceType};
use crate::modules::Module;
use crate::network::tcp::service::{self as tcp_service};
use crate::network::try_send_rpc;
use crate::{error, info};

use tokio::{
    sync::mpsc::Sender,
    time::{self, Duration},
};

pub fn start(conf: Config, mut tm: Module, all_srv_close_sender: Sender<()>) {
    tokio::spawn(async move {
        let mut log = build_logger("rpc_client_hub.log");
        info!(log, "[rpc_client_hub]: service=start");

        let mut smreceiver_chan = tm.take_smreceiver_chan().unwrap();
        let mut smreceiver = tm.take_smreceiver().unwrap();
        let mut gs = tm.take_game_state().unwrap();
        let _host_id = gs.get_host_id() as u64; //本服务的 hostid
        let mut heart_beat = time::interval(Duration::from_millis(1000));

        let mut rpc_client_srv = tcp_service::build(
            ServiceType::RPCCLIENT,
            conf,
            log.clone(),
            String::new(),
            tm.spawn_smsender(),
            tm.spawn_smsender_chan(),
        );

        let mut delay_msg = HashMap::new();
        let mut proceeding_connections = HashMap::new();
        loop {
            tokio::select! {
                // for tcp connection
                res = smreceiver_chan.recv() => {
                    if let Some((vfd,sender)) = res {
                        gs.add_vfd(vfd,sender.clone());
                        info!(log,"[rpc_client_hub]: new rpc client connection channel: vfd={}",vfd);
                        proceeding_connections.insert(vfd, 2); //连接已完成

                        //清空 delay 的消息
                        if let Some(delay_msg_v) = delay_msg.remove(&vfd) {
                            info!(log,"[rpc_client_hub]: new rpc client connection delay messages: vfd={},num={}",vfd,delay_msg.len());
                            for (_mtype, session, pto) in delay_msg_v {
                                if let Err(err) = try_send_rpc(&sender, session, pto) {
                                    error!(log,"[rpc_client_hub]: delay=true,try_send_rpc={}",err);
                                }
                            }
                        }
                    } else {
                        error!(log,"[rpc_client_hub]: smreceiver_chan=close");
                        break;
                    }
                },
                res = smreceiver.recv() => {
                    if let Some((msg_type, session, pto)) = res {
                        let (proto_id,_proto_name) = pto.inner_info();
                        if _host_id == session {
                            //rpc服务消息不能发送给本服务
                            error!(log,"[rpc_client_hub]: route_self=true,proto_id={}",proto_id);
                        } else {
                            // :TODO: 注意,这里不是用来接收 rpc 对端的消息的, 而是接收 tm 在这个服务的外面接收的消息.
                            // rpc 的client连接只做发送, 对端回来的消息不做处理
                            match msg_type {
                                MessageType::Rpc => {
                                    if let Some(sender) = gs.get_sender(session) {
                                        if let Err(err) = try_send_rpc(&sender, session, pto) {
                                            error!(log,"[rpc_client_hub]: try_send_rpc={}",err);
                                        }
                                    } else {
                                        let val = proceeding_connections.get(&session);
                                        if val.is_none() {
                                            //没有对应的 rpc 客户端连接
                                            proceeding_connections.insert(session,1); //连接未完成
                                            let idenfity = session;
                                            let addr = match &pto {
                                                ProtoType::RpcSend(inner) => {
                                                    &inner.to_addr
                                                },
                                                ProtoType::RpcResp(inner) => {
                                                    &inner.to_addr
                                                }
                                                _ => {
                                                    ""
                                                }
                                            };
                                            if addr.len() != 0 {
                                                if let Err(err) = rpc_client_srv.new_client_service(addr, idenfity).await {
                                                    error!(log,"[rpc_client_hub]: new_client_service={}",err);
                                                    proceeding_connections.remove(&idenfity); //清理标识
                                                }
                                            } else {
                                                error!(log,"[rpc_client_hub]: wrong_addr={}",addr);
                                                proceeding_connections.remove(&idenfity); //清理标识
                                            }
                                        } else {
                                            let res = *(val.unwrap());
                                            if res == 1 {
                                                // connection is not finished
                                                info!(log,"[rpc_client_hub]: connecting to {}",session);
                                            } else if res == 2 {
                                                // connection is finished, but something is wrong
                                                error!(log,"[rpc_client_hub]: new_client_service={},something is wrong.",session);
                                            }
                                        }
                                        info!(log,"[rpc_client_hub]: rpc client connection start: vfd={}",session);

                                        let delay_msg_v =
                                        if let Some(delay_msg_v) = delay_msg.get_mut(&session) {
                                            delay_msg_v
                                        } else {
                                            let delay_msg_v = Vec::new();
                                            delay_msg.insert(session, delay_msg_v);
                                            delay_msg.get_mut(&session).unwrap()
                                        };
                                        if delay_msg_v.len() < 500 {
                                            delay_msg_v.push((msg_type, session, pto));
                                        } else {
                                            info!(log,"[rpc_client_hub]: too many delay message,dumped. vfd={}",session);
                                        }
                                    }
                                },
                                MessageType::SocketClosed => {
                                    gs.delete_vfd(session);
                                    info!(log,"[rpc_client_hub]: rpc client connection close: vfd={}",session);
                                    proceeding_connections.remove(&session); //清理标识
                                },
                                _ => {
                                    error!(log,"[rpc_client_hub]: unsupport_mtype={:?},session={},proto_id={}",msg_type,session,proto_id);
                                },
                            }
                        }
                    } else {
                        error!(log,"[rpc_client_hub]: smreceiver=close");
                        break;
                    }
                },
                _ = heart_beat.tick() => {
                    //println!("rpc_client_hub service heart_beat");
                }
            }
        }
        drop(all_srv_close_sender);
        info!(log, "[rpc_client_hub]: service=stop");
    });
}

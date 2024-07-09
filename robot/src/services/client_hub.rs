use cable::config::Config;
use cable::logger::build_logger;
use cable::message::{MessageType, ServiceType};
use cable::modules::Module;
use cable::network::tcp::service::{self as tcp_service};
use cable::protos::*;
use cable::{error, info};

use tokio::{
    sync::mpsc::Sender,
    time::{self, Duration},
};

pub fn start(conf: Config, mut tm: Module, all_srv_close_sender: Sender<()>) {
    tokio::spawn(async move {
        let mut log = build_logger("tcp_client_hub.log");
        info!(log, "[tcp_client_hub]: service=start");

        let mut smreceiver_chan = tm.take_smreceiver_chan().unwrap();
        let mut smreceiver = tm.take_smreceiver().unwrap();
        let mut gs = tm.take_game_state().unwrap();

        let mut heart_beat = time::interval(Duration::from_millis(1000));

        let service_addr = conf.get_string("service_addr").unwrap();
        let robot_num = conf.get_int("robot_num").unwrap();
        println!("robot_num={}", robot_num);
        let mut tcp_client_srv = tcp_service::build(
            ServiceType::TCP,
            conf.clone(),
            log.clone(),
            String::new(),
            tm.spawn_smsender(),
            tm.spawn_smsender_chan(),
        );

        let mut is_init = false;
        let mut connected_num = 0;
        loop {
            tokio::select! {
                // for tcp connection
                res = smreceiver_chan.recv() => {
                    if let Some((vfd,sender)) = res {
                        gs.add_vfd(vfd,sender.clone());
                        connected_num = connected_num + 1;
                        info!(log,"[tcp_client_hub]: new tcp client connection channel: vfd={}, connected_num={}",vfd,connected_num);

                        //:TODO:新连接发起第一个协议,登录 (或通过#[cfg()]配置 GameState 的 robot 方法)
                        let mut s_login = S2cLogin::default();
                        s_login.account = format!("robot_{connected_num}");
                        s_login.passwd = "123456".to_string();
                        s_login.version = version().to_string();
                        let pto = ProtoType::S2cLogin(s_login);
                        let (proto_id,_name) = pto.inner_info();
                        if let Err(err) = sender.send((MessageType::Tcp,vfd,pto)).await {
                            error!(log,"[tcp_client_hub]: send SLogin to connection failed: vfd={},proto_id={},err={:?}",vfd,proto_id,err);
                        }
                    } else {
                        error!(log,"[tcp_client_hub]: smreceiver_chan=close");
                        break;
                    }
                },
                res = smreceiver.recv() => {
                    if let Some((msg_type, session, pto)) = res {
                        let (proto_id,_) = pto.inner_info();
                        match msg_type {
                            MessageType::Tcp => {
                                if gs.get_sender(session).is_some() {
                                    // :TODO: 通过#[cfg()]配置 GameState 的 robot 方法
                                    if let Err(err) = gs.robot_dispatch(msg_type, session, pto) {
                                        info!(log,"[tcp_client_hub]: err={}, vfd={}", err, session);
                                    }
                                } else {
                                    info!(log,"[tcp_client_hub]: too many delay message,dumped. vfd={}",session);
                                }
                            },
                            MessageType::SocketClosed => {
                                gs.delete_vfd(session);
                                info!(log,"[tcp_client_hub]: tcp client connection close: vfd={}",session);
                            },
                            _ => {
                                error!(log,"[tcp_client_hub]: unsupport_mtype={:?},session={},proto_id={}",msg_type,session,proto_id);
                            },
                        }
                    } else {
                        error!(log,"[tcp_client_hub]: smreceiver=close");
                        break;
                    }
                },
                _ = heart_beat.tick() => {
                    //println!("tcp_client_hub service heart_beat");
                    if !is_init {
                        for idenfity in 0..robot_num {
                            if let Err(err) = tcp_client_srv.new_client_service(service_addr, idenfity as u64 + 1).await {
                                error!(log,"[tcp_client_hub]: new_client_service={}",err);
                            }
                        }
                        is_init = true;
                    }
                }
            }
        }
        drop(all_srv_close_sender);
        info!(log, "[tcp_client_hub]: service=stop");
    });
}

use crate::config::Config;
use crate::logger::build_logger;
use crate::message::MessageType;
use crate::modules::Module;
use crate::{error, info};

use chrono::Local;
use tokio::{
    sync::mpsc::Sender,
    time::{self, Duration},
};

pub fn start(conf: Config, mut tm: Module, mut rpcm: Module, all_srv_close_sender: Sender<()>) {
    tokio::spawn(async move {
        let mut log = build_logger("game_hub.log");
        info!(log, "[game_hub]: service=start");

        let mut smreceiver_chan = tm.take_smreceiver_chan().unwrap();
        let mut smreceiver = tm.take_smreceiver().unwrap();
        let mut gs = tm.take_game_state().unwrap();

        let mut rpc_smreceiver_chan = rpcm.take_smreceiver_chan().unwrap();
        let mut rpc_smreceiver = rpcm.take_smreceiver().unwrap();
        let mut rpc_gs = rpcm.take_game_state().unwrap();

        let fps = conf.get_int("fps").unwrap_or(10); //fps 默认为 10 帧,即定时器每一tick的时间为 1000/10 毫秒
        let mut heart_beat = time::interval(Duration::from_millis(1000 / fps as u64));
        loop {
            tokio::select! {
                // for tcp connection
                res = smreceiver_chan.recv() => {
                    if let Some((vfd,sender)) = res {
                        gs.add_vfd(vfd,sender);
                        info!(log,"[game_hub]: new tcp connection channel: vfd={}",vfd);
                    } else {
                        error!(log,"[game_hub]: smreceiver_chan=close");
                        break;
                    }
                },
                res = smreceiver.recv() => {
                    if let Some((msg_type, session, pto)) = res {
                        if msg_type != MessageType::SocketClosed {
                            if let Err(err) = gs.dispatch(msg_type, session, pto) {
                                error!(log,"[game_hub]: dispatch=failed,msg_type={:?},session={},err={}",msg_type,session,err);
                            }
                        } else {
                            rpc_gs.delete_vfd(session);
                            info!(log,"[game_hub]: tcp connection close: vfd={}",session);
                        }
                    } else {
                        error!(log,"[game_hub]: smreceiver=close");
                        break;
                    }
                },
                // for rpc connection
                //对于 rpc, rpc_gs 仅维护连接的 sender; 其余消息路由到 gs 处理
                res = rpc_smreceiver_chan.recv() => {
                    if let Some((vfd,sender)) = res {
                        rpc_gs.add_vfd(vfd,sender);
                        info!(log,"[game_hub]: new rpc connection channel: vfd={}",vfd);
                    } else {
                        error!(log,"[game_hub]: rpc_smreceiver_chan=close");
                        break;
                    }
                },
                res = rpc_smreceiver.recv() => {
                    if let Some((msg_type, session, pto)) = res {
                        if msg_type != MessageType::SocketClosed {
                            if let Err(err) = gs.rpc_dispatch(msg_type, session, pto) {
                                error!(log,"[game_hub]: rpc_dispatch=failed,msg_type={:?},session={},err={}",msg_type,session,err);
                            }
                        } else {
                            rpc_gs.delete_vfd(session);
                            info!(log,"[game_hub]: rpc connection close: vfd={}",session);
                        }
                    } else {
                        error!(log,"[game_hub]: rpc_smreceiver=close");
                        break;
                    }
                },
                _ = heart_beat.tick() => {
                    let now_ms = Local::now().timestamp_millis();
                    gs.update_timer(now_ms);
                }
            }
        }
        drop(all_srv_close_sender);
        info!(log, "[game_hub]: service=stop");
    });
}

use crate::config::Config;
use crate::info;
use crate::logger::build_logger;
use crate::message::{SMSender, ServiceType};
use crate::modules::Module;
use crate::states::GameState;
use tokio::sync::mpsc;

mod game_hub;
mod rpc_client_hub;
mod tcp_hub;

pub fn start(conf: Config) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _ = rt.block_on(async move {
        run_game_server(conf).await;
    });
}

async fn run_game_server(conf: Config) {
    let mut log = build_logger("serivces.log");
    info!(log, "[run_game_server]: service=start");

    //每个模块的服务都有一个引用,模块服务结束时,各自的引用减一
    let (all_srv_close_sender, mut all_srv_close_receiver) = mpsc::channel::<()>(1);

    let service_type = conf.get_string("service_type").unwrap();
    let service_type: ServiceType = ServiceType::from(service_type.as_str());
    assert!(service_type != ServiceType::UNKNOW);
    //tcp 玩家网络连接服务
    let mut tm = new_tcp_module(service_type, conf.clone(), "tcp_module", "game_state.log");
    if service_type == ServiceType::TCP {
        let service_addr = conf.get_string("service_addr").unwrap();
        let is_ws = conf.get_bool("is_ws");
        tcp_hub::start(
            ServiceType::TCP,
            conf.clone(),
            is_ws,
            service_addr.clone(),
            "tcp_hub.log",
            tm.spawn_smsender(),
            tm.spawn_smsender_chan(),
            all_srv_close_sender.clone(),
        );
    }

    //rpc 跨机接收服务
    let rpcm = new_rpc_module(
        ServiceType::RPC,
        conf.clone(),
        "rpc_module",
        "rpc_state.log",
    );
    let rpc_service_addr = conf.get_string("rpc_service_addr").unwrap();
    tcp_hub::start(
        ServiceType::RPC,
        conf.clone(),
        false,
        rpc_service_addr.clone(),
        "rcp_hub.log",
        rpcm.spawn_smsender(),
        rpcm.spawn_smsender_chan(),
        all_srv_close_sender.clone(),
    );

    //rpc 跨机发送服务
    let rpc_clientm = new_rpc_client_module(
        ServiceType::RPCCLIENT,
        conf.clone(),
        "rpc_client_module",
        "rpc_client_state.log",
    );
    let rpc_sender = rpc_clientm.spawn_smsender();
    rpc_client_hub::start(conf.clone(), rpc_clientm, all_srv_close_sender.clone());

    tm.get_game_state().set_rpc_sender(rpc_sender);
    game_hub::start(conf.clone(), tm, rpcm, all_srv_close_sender.clone());

    //等待其他服务停止
    drop(all_srv_close_sender);
    let _ = all_srv_close_receiver.recv().await;
    info!(log, "[run_game_server]: service=ended");
}

pub fn new_tcp_module(
    service_type: ServiceType,
    conf: Config,
    module_name: &str,
    log_name: &str,
) -> Module {
    let sender_size = conf.get_int("tcp_msg_chan_size").unwrap() as usize;
    let sender_chan_size = conf.get_int("conn_chan_size").unwrap() as usize;
    let host_id = conf.get_int("host_id").unwrap();
    let gs = GameState::new(service_type, conf, host_id, log_name);

    Module::new(module_name.to_owned())
        .with_sender(sender_size)
        .with_sender_chan(sender_chan_size)
        .with_game_state(gs)
}

fn new_rpc_module(
    service_type: ServiceType,
    conf: Config,
    module_name: &str,
    log_name: &str,
) -> Module {
    new_tcp_module(service_type, conf, module_name, log_name)
}

fn new_rpc_client_module(
    service_type: ServiceType,
    conf: Config,
    module_name: &str,
    log_name: &str,
) -> Module {
    new_tcp_module(service_type, conf, module_name, log_name)
}

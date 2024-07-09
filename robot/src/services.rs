use cable::config::Config;
use cable::info;
use cable::logger::build_logger;
use cable::message::ServiceType;
use cable::services::new_tcp_module;
use tokio::sync::mpsc;

pub mod client_hub;

pub fn start(conf: Config) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _ = rt.block_on(async move {
        run_robot_server(conf).await;
    });
}

async fn run_robot_server(conf: Config) {
    let mut log = build_logger("robot_serivces.log");
    info!(log, "[run_robot_server]: service=start");

    //每个模块的服务都有一个引用,模块服务结束时,各自的引用减一
    let (all_srv_close_sender, mut all_srv_close_receiver) = mpsc::channel::<()>(1);

    let tm = new_tcp_module(
        ServiceType::TCPROBOT,
        conf.clone(),
        "tcp_module",
        "game_state.log",
    );
    client_hub::start(conf, tm, all_srv_close_sender.clone());

    //等待其他服务停止
    drop(all_srv_close_sender);
    let _ = all_srv_close_receiver.recv().await;
    info!(log, "[run_robot_server]: service=ended");
}

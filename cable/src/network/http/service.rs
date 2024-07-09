use super::{ChanHttpProtoSenderOp, HttpProtoType};
use std::future::Future;
use std::net::SocketAddr;

use crate::logger::build_logger;
use crate::{error, info};
use std::convert::Infallible;
use tokio::sync::oneshot;
use warp::Filter;

const LOG_NAME: &str = "http.log";

fn with_sender(
    sender: ChanHttpProtoSenderOp,
) -> impl Filter<Extract = (ChanHttpProtoSenderOp,), Error = Infallible> + Clone {
    warp::any().map(move || sender.clone())
}

pub async fn start_service(
    addr: SocketAddr,
    shutdown: impl Future,
    chan_out: ChanHttpProtoSenderOp,
) {
    // get /req/server/all
    let handler_req_server_all = warp::get()
        .and(warp::path!("req" / "server" / "all"))
        .map(|| "name: s1, host: s1.xxx.com:8081");

    let chan_out_req_server = chan_out.clone();
    // get /req/server/:u32
    // builds on example2 but adds custom error handling
    let handler_req_server = warp::get()
        .and(warp::path!("req" / "server" / u32))
        .and(with_sender(chan_out_req_server))
        .then(req_server)
        .map(|res| res);

    // get /gm/:string
    let chan_out_gm_add_item = chan_out.clone();
    let handler_gm_add_item = warp::get()
        .and(warp::path("gm"))
        .and(warp::path::param())
        .and(with_sender(chan_out_gm_add_item))
        .then(gm)
        .map(|res| res);

    let routes = handler_req_server_all
        .or(handler_req_server)
        .or(handler_gm_add_item);

    let mut log = build_logger(LOG_NAME);
    tokio::select! {
        _ = warp::serve(routes).run(addr) => {
            error!(log,"http.run closed.");
        }
        _ = shutdown => {
            info!(log,"http.run shut down.");
        }
    }
}

async fn req_server(hostid: u32, chan_out: ChanHttpProtoSenderOp) -> String {
    let (optx, oprx) = oneshot::channel();
    tokio::spawn(async move {
        //这里不需要 try_send, 如果 channel 阻塞,就让它一直阻塞, 因为send出去的是 oneshot 的 chan
        if let Err(err) = chan_out
            .send((HttpProtoType::ReqServerInfo(hostid), optx))
            .await
        {
            let mut log = build_logger(LOG_NAME);
            error!(
                log,
                "req_server, chan full: hostid={},err={:?}", hostid, err
            );
        }
    });
    match oprx.await {
        Ok(hpt) => {
            format!("success.\n{:?}", hpt)
        }
        Err(err) => {
            let mut log = build_logger(LOG_NAME);
            error!(log, "/req/server/{},err={:?}", hostid, err);
            format!("failed,{}", err)
        }
    }
}

async fn gm(cmdstr: String, chan_out: ChanHttpProtoSenderOp) -> String {
    let (optx, oprx) = oneshot::channel();
    let cmdstr2 = cmdstr.clone();
    tokio::spawn(async move {
        //这里不需要 try_send, 如果 channel 阻塞,就让它一直阻塞, 因为send出去的是 oneshot 的 chan
        if let Err(err) = chan_out
            .send((HttpProtoType::ReqGM(cmdstr.clone()), optx))
            .await
        {
            let mut log = build_logger(LOG_NAME);
            error!(
                log,
                "gm_add_item, chan full: cmdstr={},err={:?}", cmdstr, err
            );
        }
    });
    match oprx.await {
        Ok(hpt) => {
            format!("success.\n{:?}", hpt)
        }
        Err(err) => {
            let mut log = build_logger(LOG_NAME);
            error!(log, "/gm/{},err={:?}", cmdstr2, err);
            format!("failed,{}", err)
        }
    }
}

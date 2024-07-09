use std::ffi::c_void;

use super::{Communicate, TcpState, TimerState};
use crate::config::Config;
use crate::logger::{build_logger, Outter};
use crate::luautil;
use crate::message::{MessageType, ProtoType, SMSender, ServiceType};
use crate::{error, info};
use crate::{network, protos::*};
use rlua::{Function, Lua, Table};

pub struct GameState {
    host_id: i32,
    pub log: Outter,
    rpc: Option<SMSender>,
    pub lua_state: Option<Lua>,
    tcp_state: Box<TcpState>,
    timer_state: Box<TimerState>,
}

impl GameState {
    pub fn new(service_type: ServiceType, conf: Config, host_id: i32, log_name: &str) -> Self {
        let log = build_logger(log_name);

        let tcp_state = Box::new(TcpState::new());
        let fps = conf.get_int("fps").unwrap_or(10);
        let timer_state = Box::new(TimerState::new(fps));
        //初始化lua虚拟机
        let lua_state = if service_type == ServiceType::TCP
            || service_type == ServiceType::TCPROBOT
            || service_type == ServiceType::DB
        {
            let lua_state = luautil::init_lua(service_type, conf.clone()).unwrap();
            //注册 tcp 消息到脚本层的处理函数
            let tmp_tcp_state = &(*tcp_state) as *const TcpState as *mut c_void;
            luautil::init_tcp_state(&lua_state, tmp_tcp_state).unwrap();
            //注册 timer 消息到脚本层的处理函数
            let tmp_timer_state = &(*timer_state) as *const TimerState as *mut c_void;
            luautil::init_timer_state(&lua_state, tmp_timer_state).unwrap();
            //脚本层入口,完成脚本层初始化
            let lua_state = luautil::call_entry(lua_state, conf.clone());
            Some(lua_state)
        } else {
            None
        };

        GameState {
            host_id,
            log,
            rpc: None,
            lua_state,
            tcp_state,
            timer_state,
        }
    }

    pub fn get_host_id(&self) -> i32 {
        self.host_id
    }

    pub fn add_vfd(&mut self, vfd: u64, smsender: SMSender) {
        info!(self.log, "[add_vfd]: vfd={vfd}");
        (*self.tcp_state).register(vfd, smsender);
    }

    pub fn delete_vfd(&mut self, vfd: u64) {
        info!(self.log, "[delete_vfd]: vfd={vfd}");
        (*self.tcp_state).unregister(vfd);
    }

    pub fn get_sender(&mut self, vfd: u64) -> Option<&SMSender> {
        (*self.tcp_state).get(vfd)
    }

    pub fn set_rpc_sender(&mut self, rpc_sender: SMSender) {
        assert!(self.rpc.is_none());
        self.rpc = Some(rpc_sender.clone());

        luautil::init_rpc_send(self, rpc_sender);
    }

    pub fn get_rpc_sender(&mut self) -> Option<&SMSender> {
        self.rpc.as_ref()
    }

    pub fn update_timer(&mut self, now: i64) {
        if let Some(trigger) = self.timer_state.update(now) {
            self.lua_state.as_ref().unwrap().context(|ctx| {
                let _timer_msg: Function = ctx.globals().get("_timer_msg").unwrap();
                let _ = _timer_msg.call::<Vec<u64>, ()>(trigger);
            });
        }
    }

    pub fn dispatch(
        &mut self,
        _msg_type: MessageType,
        vfd: u64,
        pto: ProtoType,
    ) -> crate::Result<()> {
        let (proto_id, proto_name) = pto.inner_info();
        self.lua_state.as_ref().unwrap().context(|ctx| {
            let _tcp_msg: Function = ctx.globals().get("_tcp_msg").unwrap();
            match pto.encode_to_lua(ctx) {
                Ok(t) => {
                    let _ = _tcp_msg
                        .call::<(u64, u32, &str, Table), ()>((vfd, proto_id, proto_name, t));
                }
                Err(err) => {
                    info!(
                        self.log,
                        "[dispatch]: encode_to_lua=failed,vfd={vfd},proto_id={proto_id},err={err}"
                    );
                }
            }
        });
        Ok(())
    }

    pub fn rpc_dispatch(
        &mut self,
        _msg_type: MessageType,
        _vfd: u64,
        pto: ProtoType,
    ) -> crate::Result<()> {
        let (proto_id, proto_name) = pto.inner_info();
        self.lua_state.as_ref().unwrap().context(|ctx| {
            let _rpc_msg: Function = ctx.globals().get("_rpc_msg").unwrap();
            match pto {
                ProtoType::RpcSend(p) => {
                    let _ = _rpc_msg.call::<(bool, i32, String, u64, String, String), ()>((
                        true,
                        p.from_host,
                        p.from_addr,
                        p.session,
                        p.func,
                        p.args,
                    ));
                }
                ProtoType::RpcResp(p) => {
                    let _ = _rpc_msg.call::<(bool, i32, String, u64, String, String), ()>((
                        false,
                        p.from_host,
                        p.from_addr,
                        p.session,
                        p.func,
                        p.args,
                    ));
                }
                _ => {
                    println!("unhandle rpc proto: {proto_id},{proto_name}");
                }
            }
        });
        Ok(())
    }

    pub fn robot_dispatch(
        &mut self,
        msg_type: MessageType,
        vfd: u64,
        pto: ProtoType,
    ) -> crate::Result<()> {
        let (proto_id, proto_name) = pto.inner_info();
        println!(
            "[robot_dispatch]: msg_type={:?},vfd={vfd},proto_id={proto_id},proto_name={proto_name}",
            msg_type
        );
        Ok(())
    }

    fn _test_dispatch(
        &mut self,
        _msg_type: MessageType,
        vfd: u64,
        _pto: ProtoType,
    ) -> crate::Result<()> {
        // let mut c_inventory_req = CInventoryReq::default();
        // c_inventory_req.tag = 1;
        // let mut items = vec![];
        // for i in 0..20 {
        //     let mut item = Item::default();
        //     item.uid = i as u64;
        //     item.id = i as u32;
        //     items.push(item);
        // }
        // c_inventory_req.items = items;
        // let pto = ProtoType::CInventoryReq(c_inventory_req);
        // let sender = self.get_sender(vfd).unwrap();
        // network::try_send(&sender, vfd, pto)?;
        Ok(())
    }

    //假设当前配置只有 1,2 两个 host_id
    pub fn test_rpc_send(&mut self) -> crate::Result<()> {
        let mut rsend = RpcSend::default();
        if self.host_id == 1 {
            rsend.from_host = 1;
            rsend.from_addr = "0.0.0.0:8182".to_string();

            rsend.to_host = 2;
            rsend.to_addr = "0.0.0.0:8184".to_string();

            rsend.session = 100001;
            rsend.func = "func_rpc_test".to_string();
            rsend.args = "11,22,33".to_string();
        } else {
            rsend.from_host = 2;
            rsend.from_addr = "0.0.0.0:8184".to_string();

            rsend.to_host = 1;
            rsend.to_addr = "0.0.0.0:8182".to_string();

            rsend.session = 200001;
            rsend.func = "func_rpc_test".to_string();
            rsend.args = "33,11,22".to_string();
        }
        let to_host = rsend.to_host;
        let pto = ProtoType::RpcSend(rsend);
        let sender = self.get_rpc_sender().unwrap();
        network::try_send_rpc(sender, to_host as u64, pto)?;

        Ok(())
    }
}

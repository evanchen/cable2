use crate::config::Config;
use crate::logger::{build_logger, Outter};
use crate::message::{SMSender, ServiceType};
use crate::states::{Communicate, GameState, TcpState, TimerState};
use crate::{debug, error, info, warning};
use crate::{network, protos::*};
use chrono::Local;
use rlua::{LightUserData, Lua, Table, ToLua};
use std::collections::HashMap;
use std::ffi::c_void;
use std::fs::read_to_string;

pub fn init_lua(service_type: ServiceType, conf: Config) -> rlua::Result<Lua> {
    let logic_path = conf.get_string("logic_path").unwrap();
    let lua_state = unsafe { Lua::new_with_debug() };
    lua_state.context(|ctx| {
        //初始化全局变量
        let globals = ctx.globals();
        //所有全局变量或函数,都注册在 xlib 这个table下
        //====================== 注册供脚本层使用的全局变量 ======================
        //业务逻辑脚本层代码根目录
        let xlib = ctx.create_table()?;
        xlib.set("pwd", logic_path.as_str())?;

        let host_id = conf.get_int("host_id").unwrap();
        xlib.set("host_id", host_id)?;

        if let Some(rpc_service_addr) = conf.get_string("rpc_service_addr") {
            xlib.set("rpc_service_addr", rpc_service_addr.to_owned())?;
        }

        let log_level = conf.get_int("log_level").unwrap();
        xlib.set("log_level", log_level)?;
        //====================== 注册供脚本层调用的函数 ======================
        //时间相关
        //毫秒 10-3秒
        let time_ms = ctx.create_function(|_, ()| {
            let ms = Local::now().timestamp_millis();
            Ok(ms)
        })?;
        xlib.set("time_ms", time_ms)?;
        //纳秒（10-9秒）
        let time_ns = ctx.create_function(|_, ()| {
            let ns = Local::now().timestamp_nanos();
            Ok(ns)
        })?;
        xlib.set("time_ns", time_ns)?;

        //日志相关
        let mut alllogs: HashMap<String, Outter> = HashMap::new();
        let log = ctx.create_function_mut(
            move |_, (log_name, lvl, log_str): (String, String, String)| {
                if !alllogs.contains_key(&log_name) {
                    let newlog = build_logger(&log_name);
                    alllogs.insert(log_name.clone(), newlog);
                }
                let log = alllogs.get_mut(&log_name).unwrap();
                match lvl.as_str() {
                    "info" => {
                        info!(log, "{}", log_str);
                    }
                    "error" => {
                        error!(log, "{}", log_str);
                    }
                    "warn" => {
                        warning!(log, "{}", log_str);
                    }
                    _ => {
                        debug!(log, "{}", log_str);
                    }
                }
                Ok(())
            },
        )?;
        xlib.set("log", log)?;

        //加密相关

        //宿主层和脚本层协议解码编码相关
        let serialize_table_to_string = ctx.create_function(|ctx, t: Table| {
            let s = serialize_table_to_string(ctx, t)?;
            match String::from_utf8(s) {
                Ok(s) => Ok(s),
                Err(err) => Err(rlua::Error::RuntimeError(err.to_string())),
            }
        })?;
        xlib.set("table2str", serialize_table_to_string)?;

        globals.set("xlib", xlib)?;

        let service_type: String = service_type.into();
        globals.set("service_type", service_type)?;
        Ok(())
    })?;
    Ok(lua_state)
}

//脚本层入口
pub fn call_entry(lua: Lua, conf: Config) -> Lua {
    let logic_path = conf.get_string("logic_path").unwrap();
    let entry = format!("{}/main.lua", logic_path);
    let lua_script = read_to_string(entry).unwrap();
    lua.context(|ctx| ctx.load(&lua_script).exec().unwrap());
    lua
}

pub fn init_tcp_state(lua_state: &Lua, tcp_state: *mut c_void) -> rlua::Result<()> {
    lua_state.context(|ctx| {
        let tmpstate = rlua::LightUserData(tcp_state);
        let globals = ctx.globals();
        globals.set("tcp_state", tmpstate)?;
        let tcp_send = ctx.create_function(
            |ctx, (vfd, proto_id, proto_name, body): (u64, i32, String, Table)| {
                let tcp_state: LightUserData = ctx.globals().get("tcp_state").unwrap();
                let tcp_state = tcp_state.0 as *mut TcpState;
                let tcp_state = unsafe { &mut (*tcp_state) };
                if let Some(sender) = (*tcp_state).conn_map().get(&vfd) {
                    match ProtoType::from_id(proto_id) {
                        Some(pto) => {
                            let pto = pto.decode_from_lua(body)?;
                            let _ = network::try_send(sender, vfd, pto);
                        }
                        None => {
                            println!("[tcp_send]:sender=failed,vfd={vfd},proto_id={proto_id},proto_name={proto_name}");
                        }
                    }
                } else {
                    println!("[tcp_send]:sender=nosender,vfd={vfd},proto_id={proto_id},proto_name={proto_name}");
                }
                Ok(())
            },
        )?;
        let xlib: Table = globals.get("xlib")?;
        xlib.set("tcp_send", tcp_send)?;
        Ok(())
    })?;
    Ok(())
}

pub fn init_timer_state(lua_state: &Lua, timer_state: *mut c_void) -> rlua::Result<()> {
    lua_state.context(|ctx| {
        let tmpstate = rlua::LightUserData(timer_state);
        let globals = ctx.globals();
        globals.set("timer_state", tmpstate)?;

        let xlib: Table = globals.get("xlib")?;

        let add_timer = ctx.create_function(|ctx, (begin, freq): (i64, i64)| {
            let timer_state: LightUserData = ctx.globals().get("timer_state").unwrap();
            let timer_state = timer_state.0 as *mut TimerState;
            let timer_state = unsafe { &mut (*timer_state) };
            let id = timer_state.add_timer(begin, freq);
            Ok(id)
        })?;
        xlib.set("add_timer", add_timer)?;

        let remove_timer = ctx.create_function(|ctx, id: u64| {
            let timer_state: LightUserData = ctx.globals().get("timer_state").unwrap();
            let timer_state = timer_state.0 as *mut TimerState;
            let timer_state = unsafe { &mut (*timer_state) };
            timer_state.remove_timer(id);
            Ok(())
        })?;
        xlib.set("remove_timer", remove_timer)?;
        Ok(())
    })?;
    Ok(())
}

pub fn init_rpc_send(gate_state: &mut GameState, rpc_sender: SMSender) {
    let mut log = gate_state.log.clone();
    //初始化 lua 的 rpc_send 函数
    gate_state.lua_state.as_ref().unwrap().context(|ctx| {
        let xlib: Table = ctx.globals().get("xlib").unwrap();
        let rpc_send = ctx
            .create_function_mut(
                move |ctx,
                      (is_send, from_host, from_addr, to_host, to_addr, session, func, args): (
                    bool,
                    i32,
                    String,
                    i32,
                    String,
                    u64,
                    String,
                    Table,
                )| {
                    if is_send {
                        let mut rsend = RpcSend::default();
                        rsend.from_host = from_host;
                        rsend.from_addr = from_addr;

                        rsend.to_host = to_host;
                        rsend.to_addr = to_addr;

                        rsend.session = session;
                        rsend.func = func;
                        let s = serialize_table_to_string(ctx, args)?;
                        let s = match String::from_utf8(s) {
                            Ok(s) => s,
                            Err(err) => return Err(rlua::Error::RuntimeError(err.to_string())),
                        };
                        rsend.args = s;

                        let pto = ProtoType::RpcSend(rsend);
                        if let Err(err) = network::try_send_rpc(&rpc_sender, to_host as u64, pto) {
                            error!(log, "{}", err);
                        }
                    } else {
                        let mut rsend = RpcResp::default();
                        rsend.from_host = from_host;
                        rsend.from_addr = from_addr;

                        rsend.to_host = to_host;
                        rsend.to_addr = to_addr;

                        rsend.session = session;
                        rsend.func = func;
                        let s = serialize_table_to_string(ctx, args)?;
                        let s = match String::from_utf8(s) {
                            Ok(s) => s,
                            Err(err) => return Err(rlua::Error::RuntimeError(err.to_string())),
                        };
                        rsend.args = s;

                        let pto = ProtoType::RpcResp(rsend);
                        if let Err(err) = network::try_send_rpc(&rpc_sender, to_host as u64, pto) {
                            error!(log, "{}", err);
                        }
                    }

                    Ok(())
                },
            )
            .unwrap();
        xlib.set("rpc_send", rpc_send).unwrap();
    });
}

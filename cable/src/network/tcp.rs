pub mod client_service;
pub mod read;
pub mod service;
pub mod write;

//一个完整的自定义消息的头部,包括: 协议id(u32) + 协议包总长度(u32), 8个字节
pub const PROTO_HEADER_LEN: usize = 8;
//定义一个消息体长度上限为 10 mb, 包括消息头长度
pub const PROTO_BODY_MAX_LEN: usize = 10 * 1024 * 1024 - PROTO_HEADER_LEN;

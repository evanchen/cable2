use super::service::Service;
use crate::error::Error;
use crate::info;
use tokio::net::TcpStream;

impl Service {
    //作为一个 client 实例运行服务
    pub async fn new_client_service(&mut self, addr: &str, idenfity: u64) -> crate::Result<()> {
        info!(
            self.log,
            "[new_client_service]: idenfity={idenfity},start connecting to {addr}..."
        );
        match TcpStream::connect(addr).await {
            Ok(stream) => {
                self.handle_stream(stream, idenfity).await?;
                Ok(())
            }
            Err(err) => {
                let str: String = format!("[new_client_service]: idenfity={idenfity},err={}", err);
                Err(Error::Message(str))
            }
        }
    }
}

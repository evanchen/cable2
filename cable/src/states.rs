pub mod game_state;
pub use game_state::GameState;

pub mod tcp_state;
pub use tcp_state::TcpState;

pub mod timer_state;
pub use timer_state::TimerState;

use std::collections::HashMap;

pub trait Communicate<T> {
    fn register(&mut self, vfd: u64, sender: T) {
        self.conn_map().insert(vfd, sender);
        //println!("[register]: vfd={vfd}");
    }

    fn unregister(&mut self, vfd: u64) {
        self.conn_map().remove(&vfd);
        //println!("[unregister]: vfd={vfd}");
    }

    fn get(&mut self, vfd: u64) -> Option<&T> {
        self.conn_map().get(&vfd)
    }

    fn conn_map(&mut self) -> &mut HashMap<u64, T>;
}

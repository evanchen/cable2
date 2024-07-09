use chrono::Local;

//考虑到游戏业务中,对固定频率的定时器的使用,不同频率大小的定时器其实并不会很多,也不会无限大,而且一般都是短时的定时器比较多.
//所以我们这里设计的定时器,功能其实很简单,直接用过期时间做排序即可.
//如果需要支持可能无限多的定时器,需要考虑为 时间轮或最小堆,减少大量定时器的空间和排序时间.
// pub struct Timer {
//     id: u64,
//     freq: i64, //执行频率,可考虑用tick次数,这样就跟系统时间无关(每一tick为每一次update调用)
// }

pub struct TimerState {
    fps: i32,                          //祯率
    inc_id: u64,                       //递增id
    orders: Vec<(i64, u64, i64)>, //以 (timeout,id,freq) 为元素,保持有序; 为了排序更快,不选择VecDeque
    once_orders: Vec<(i64, u64, i64)>, //只执行一次的 timer
}

impl TimerState {
    pub fn new(fps: i32) -> Self {
        assert!(fps > 0);
        TimerState {
            fps,
            inc_id: 0,
            orders: Vec::with_capacity(100),
            once_orders: Vec::with_capacity(100),
        }
    }

    // begin,freq 都是以毫秒为单位
    pub fn add_timer(&mut self, begin: i64, freq: i64) -> u64 {
        if freq < 0 || begin < 0 {
            return 0;
        }

        if freq > 0 {
            //以定时频率的定时器,祯率不能超出服务器的祯率
            let tar_fps = 1000 / freq as i32;
            if tar_fps > self.fps {
                return 0;
            }
        }

        let id = self.inc_id + 1;
        self.inc_id = id;

        let now_ms = Local::now().timestamp_millis();
        let timeout = now_ms + (begin as i64);
        if freq == 0 {
            //这是一次性的定时器
            self.once_orders.push((timeout, id, freq));
            //保持有序
            self.once_orders.sort_by(|a, b| a.0.cmp(&b.0));
            //println!("[add_timer]: once=true,{:?}", self.once_orders);
        } else {
            //这是以 freq 为频率执行的定时器
            self.orders.push((timeout, id, freq));
            //保持有序
            self.orders.sort_by(|a, b| a.0.cmp(&b.0));
            //println!("[add_timer]: freq=true,{:?}", self.orders);
        }
        return id;
    }

    pub fn remove_timer(&mut self, id: u64) {
        if let Some(pos) = self
            .orders
            .iter()
            .position(|(_timeout, tid, _freq)| *tid == id)
        {
            self.orders.remove(pos);
            //println!("[remove_timer]: freq=true,{:?}", self.orders);
        }

        if let Some(pos) = self
            .once_orders
            .iter()
            .position(|(_timeout, tid, _freq)| *tid == id)
        {
            self.once_orders.remove(pos);
            //println!("[remove_timer]: once=true,{:?}", self.once_orders);
        }
    }

    pub fn update(&mut self, now: i64) -> Option<Vec<u64>> {
        let num = self.orders.len();
        if num == 0 {
            return None;
        }
        //共有多少个timeout到期了
        let mut trigger_num = 0;
        for (timeout, _id, _freq) in &self.orders {
            if *timeout <= now {
                trigger_num = trigger_num + 1;
            }
        }

        let mut trigger_num_once = 0;
        for (timeout, _id, _freq) in &self.once_orders {
            if *timeout <= now {
                trigger_num_once = trigger_num_once + 1;
            }
        }

        if trigger_num == 0 && trigger_num_once == 0 {
            return None;
        } else {
            //println!("[update]:{}", now);
            let mut trigger = Vec::with_capacity(trigger_num + trigger_num_once);
            if trigger_num > 0 {
                for i in 0..trigger_num {
                    let mut t = self.orders.get_mut(i).unwrap();
                    t.0 = now + t.2; //更新下一次触发时间
                    trigger.push(t.1);
                }
                //保持有序
                self.orders.sort_by(|a, b| a.0.cmp(&b.0));
                //println!("[update]: freq=true,{:?}", self.orders);
            }
            if trigger_num_once > 0 {
                for _i in 0..trigger_num_once {
                    let t = self.once_orders.remove(0); //从第一个移除,因为是一次性的
                    trigger.push(t.1);
                }
                //保持有序
                self.once_orders.sort_by(|a, b| a.0.cmp(&b.0));
                //println!("[update]: once=true,{:?}", self.once_orders);
            }
            Some(trigger)
        }
    }
}

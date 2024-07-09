//正在写文件需要创建 inner 对象

use crate::{error::Error, Result};
use chrono::{DateTime, Local, NaiveDate, NaiveDateTime};
use std::io::{BufRead, BufReader, Write};
use std::{
    fs::{self, create_dir_all, File, OpenOptions},
    os::unix::prelude::MetadataExt,
};

#[derive(Debug)]
pub struct Inner {
    path: String,           //文件路径
    name: String,           //文件名
    handler: Option<File>,  //文件句柄
    create_date: NaiveDate, //对象创建的时间
    size: u64,              //当前文件大小, 单位 byte
    max_size: u64,          //文件大小最大上限, 单位 byte
    roll_times: i32,        //当天文件滚动次数
}

impl Inner {
    pub fn new(path: &str, name: &str) -> Self {
        Inner {
            path: path.to_string(),
            name: name.to_string(),
            handler: None,
            create_date: Local::now().date_naive(),
            size: 0,
            max_size: 100 * 1024 * 1024, //默认100M
            roll_times: 0,
        }
    }

    pub fn get_path(&self) -> &str {
        &self.path
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn set_max_size(&mut self, max_size: u64) {
        self.max_size = max_size;
    }

    pub fn set_create_date(&mut self, new_date: DateTime<Local>) {
        self.create_date = new_date.date_naive();
    }

    pub fn write(&mut self, logstr: &str) -> Result<()> {
        //写之前先判断是否触发时间滚动文件
        self.check_date()?;
        self.actual_write(logstr)?;
        self.check_size()?;
        Ok(())
    }

    pub fn check_date(&mut self) -> Result<()> {
        if Local::now().date_naive() != self.create_date {
            self.roll()
        } else {
            Ok(())
        }
    }

    pub fn check_size(&mut self) -> Result<()> {
        if self.size >= self.max_size {
            self.roll()
        } else {
            Ok(())
        }
    }

    pub fn roll(&mut self) -> Result<()> {
        self.handler.take();
        self.size = 0;
        self.roll_times += 1;
        self.create_date = Local::now().date_naive();

        let newname = format!(
            "{}.{}-{}",
            self.path,
            Local::now().format("%Y-%m-%d %H:%M:%S%.6f").to_string(),
            self.roll_times
        );
        match fs::rename(&self.path, &newname) {
            Ok(_) => Ok(()),
            Err(err) => Err(Error::IoError(err)),
        }
    }

    pub fn dowrite(&mut self, logstr: &str) -> Result<()> {
        let fh = self.handler.as_mut().unwrap();
        let n: usize = logstr.len();
        writeln!(fh, "{}", logstr)?;
        self.size += n as u64;
        //fh.flush()?;
        println!("{logstr}"); // for debug
        Ok(())
    }

    pub fn actual_write(&mut self, logstr: &str) -> Result<()> {
        match self.handler {
            Some(_) => self.dowrite(logstr),
            None => {
                let now = Local::now();
                //尝试打开当前文件
                let mt = fs::metadata(&self.path);
                if mt.is_ok() {
                    //文件存在,读取最后一行，判断文件最后写入时间
                    let tmp: File = File::open(&self.path)?;
                    let reader = BufReader::new(tmp);
                    let mut last_line = String::from("");
                    if let Some(line) = reader.lines().last() {
                        if let Ok(lstr) = line {
                            last_line = lstr;
                        }
                    }
                    //println!("Last line: {}==={}", self.path, last_line);

                    if last_line.len() == 0 {
                        return self.create_new_file_and_write(logstr);
                    }

                    //每一行都是以 "[时间][事件][日志等级]: 日志内容" 格式写入日志文件
                    let mut need_new_file = true; //是否需要重新创建文件
                    if let Some(epos) = last_line.find(']') {
                        if epos > 1 {
                            let timestr = &last_line[1..epos];
                            match NaiveDateTime::parse_from_str(timestr, "%Y-%m-%d %H:%M:%S%.6f") {
                                Ok(last_time) => {
                                    if last_time.date() == now.date_naive() {
                                        need_new_file = false;
                                    } else {
                                        self.roll()?
                                    }
                                }
                                Err(err) => {
                                    eprintln!("{}", err);
                                }
                            }
                        }
                    }
                    //println!("-----{},{}", self.path, need_new_file);
                    if need_new_file {
                        self.create_new_file_and_write(logstr)
                    } else {
                        //不需要创建文件，则打开当前文件写入
                        match OpenOptions::new().append(true).open(&self.path) {
                            Ok(fh) => {
                                self.size = mt.unwrap().size();
                                self.handler = Some(fh);
                                self.dowrite(logstr)
                            }
                            Err(err) => Err(Error::IoError(err)),
                        }
                    }
                } else {
                    //文件不存在
                    //创建文件路径, 例如绝对或相对路径是 xxx/log/player/player.log, 可能是第一次创建文件，所以得先创建目录 xxx/log/player
                    //文件路径必须带目录分割符 ‘/’
                    println!("creating new file: {}", self.path);
                    let pos: usize = self.path.rfind('/').unwrap();
                    let (dir, _) = self.path.split_at(pos);
                    create_dir_all(dir)?;

                    self.create_new_file_and_write(logstr)
                }
            }
        }
    }

    pub fn create_new_file_and_write(&mut self, logstr: &str) -> Result<()> {
        match OpenOptions::new()
            .append(true)
            .create(true)
            .open(&self.path)
        {
            Ok(fh) => {
                self.handler = Some(fh);
                self.size = 0;
                self.roll_times = 0;

                self.dowrite(logstr)
            }
            Err(err) => Err(Error::IoError(err)),
        }
    }
}

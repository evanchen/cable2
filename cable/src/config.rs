use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};

#[derive(Debug, Clone)]
pub struct Config {
    values: HashMap<String, String>,
}

impl Config {
    pub fn new(fpath: &str) -> Self {
        let file =
            File::open(fpath).expect(format!("open config file:{fpath} should be ok").as_str());
        let reader = BufReader::new(file);

        let mut values: HashMap<String, String> = HashMap::new();

        for line in reader.lines() {
            let ln: String = line.unwrap();
            //移除注释文本, 注释文本约定为以 '#' 号开始的文本， 例如 k = v #这后面是注释
            //移除后配置行是 k = v 的格式
            let target = if let Some(pos) = ln.find('#') {
                let (target, _s2) = ln.split_at(pos);
                target
            } else {
                ln.as_str()
            };

            //如果是空行,或不是 k = v 的格式，就过滤掉
            if target.len() == 0 {
                continue;
            }

            //此时,target 就剩下 k = v
            let kv: Vec<&str> = target.split('=').collect();
            if kv.len() != 2 {
                panic!("this line content: [{}] is not a k=v format", target);
            }
            //k,v 字符串必须是前后都没有空格的
            let k: String = kv[0].trim().into();
            let v: String = kv[1].trim().into();
            values.insert(k, v);
        }
        Config { values }
    }

    pub fn get_int(&self, k: &str) -> Option<i32> {
        if let Some(v) = self.values.get(k) {
            let res: i32 = v.parse().unwrap();
            Some(res)
        } else {
            None
        }
    }

    pub fn get_float(&self, k: &str) -> Option<f32> {
        if let Some(v) = self.values.get(k) {
            let res: f32 = v.parse().unwrap();
            Some(res)
        } else {
            None
        }
    }

    pub fn get_string(&self, k: &str) -> Option<&String> {
        self.values.get(k)
    }

    pub fn get_bool(&self, k: &str) -> bool {
        if let Some(v) = self.values.get(k) {
            if v == "true" {
                return true;
            }
        }
        return false;
    }

    pub fn with(mut self, k: &str, v: &str) -> Self {
        self.values.insert(k.to_string(), v.to_string());
        self
    }
}

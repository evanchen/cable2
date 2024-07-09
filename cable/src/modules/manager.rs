use super::instance::Module;
use std::collections::HashMap;

pub struct ModuleManager {
    modules: HashMap<String, Module>,
}

impl ModuleManager {
    pub fn new() -> Self {
        ModuleManager {
            modules: HashMap::new(),
        }
    }

    pub fn register(&mut self, m: Module) -> bool {
        if self.modules.contains_key(m.name()) {
            false
        } else {
            self.modules.insert(m.name().to_owned(), m);
            true
        }
    }
}

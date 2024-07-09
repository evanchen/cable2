use md5;
use std::collections::HashMap;
use std::fs::{self, create_dir_all, File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::Command;

// #[derive(Debug,PartialEq,Clone)]
// struct ProtoInfo {
//     Name: String,
//     Mod: String,
//     Id: u32,
// }

fn main() {
    start_builds();
    fmt_generated();
}

fn start_builds() {
    //let mut config = prost_build::Config::new();
    // config.out_dir("src/"); // Specify the desired output directory
    //                         //config.file_descriptor_set_path("src/protocal.rs"); // Specify the desired file path and name
    // config
    //     .compile_protos(&["item/item.proto","inventory/inventory.proto"], &["proto"])
    //     .unwrap();

    //fs::rename("src/_.rs", "src/protocal.rs").expect("Failed to rename the file");

    let include_dir = Path::new("./proto");
    let protos = iterate_directory(include_dir);

    let out_dir = "src/output/protors";
    create_dir_all(out_dir).unwrap();

    let mut struct_names = vec![]; // {Item,Ivnentory,Player,...}
    let mut unique_names = HashMap::new(); // [Item] = item,[OtherItem] = item,...
    let mut struct_names_in_mod = HashMap::new(); //[item] = {Item,...}
    for (proto, rs) in protos {
        let proto_short = proto.replace("./proto/", "");
        generate_rs(&proto_short, &rs, include_dir.to_str().unwrap(), out_dir);

        let mod_name = rs.replace(".rs", "");
        let mut names = get_struct_name(&proto, &mut unique_names, &mod_name);
        struct_names.extend_from_slice(&names);

        names.sort(); //模块内保持有序
        struct_names_in_mod.insert(mod_name, names);
    }
    //按字母字典顺序排序,导出文件时，需要保持以字母排序的顺序，以便 git diff 看到明显的
    struct_names.sort();

    //给每个 message 结构赋值一个唯一id
    let mut name2id = HashMap::new(); // [Item] = 101
    for (id, name) in struct_names.iter().enumerate() {
        let proto_id = id + 100;
        name2id.insert(name.to_owned(), proto_id as u32);
    }

    let target1 = "src/output/allprotos.rs";
    generate_allptos(
        target1,
        &struct_names,
        &unique_names,
        &struct_names_in_mod,
        &name2id,
    );

    let target2 = "src/output/luaseri.rs";
    generate_lua_encode_decode(
        target2,
        &struct_names,
        &unique_names,
        &struct_names_in_mod,
        &name2id,
    );

    let target3 = "src/output/allprotos.lua";
    generate_allprotos_lua(target3, &struct_names, &name2id);

    //把 luaseri.rs 追加到 allprotos.rs 文件后面
    let mut fh = OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)
        .open(&target1)
        .unwrap();
    let mut file = File::open(target2).unwrap();
    let mut buffer = String::new();
    file.read_to_string(&mut buffer).unwrap();
    fh.write(buffer.as_bytes()).unwrap();
}

fn fmt_generated() {
    let output = Command::new("cargo")
        .args(&["fmt", "--all"]) // Additional arguments for `cargo fmt`
        .output()
        .expect("Failed to execute command");

    // Check if the command was successful
    if output.status.success() {
        println!("cargo fmt executed successfully!");
    } else {
        // Print any error messages or output to help with debugging
        if let Some(stderr) = String::from_utf8(output.stderr).ok() {
            eprintln!("cargo fmt failed with error: {}", stderr);
        }
    }
}

fn generate_rs(proto: &str, name: &str, from: &str, to: &str) {
    let mut config = prost_build::Config::new();
    config.out_dir(to); // Specify the desired output directory
    config.compile_protos(&[proto], &[from]).unwrap();

    let default_name = format!("{to}/_.rs");
    let rename = format!("{to}/{name}");
    // fs::rename(default_name, &rename)
    //     .expect(format!("Failed to rename the file {}", rename).as_str());
    let _ = fs::rename(default_name, &rename);
}

fn iterate_directory(dir: &Path) -> Vec<(String, String)> {
    let mut output = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();

        if path.is_dir() {
            let subdirectory_output = iterate_directory(&path);
            output.extend(subdirectory_output);
        } else if let Some(file_name) = path.file_name() {
            let file_name = file_name.to_string_lossy();
            if file_name.ends_with(".proto") {
                let proto_file = file_name.to_string();
                let rust_file = proto_file.replace(".proto", ".rs");
                output.push((path.to_string_lossy().to_string(), rust_file));
            }
        }
    }
    output
}

fn get_struct_name(
    fpath: &str,
    unique: &mut HashMap<String, String>,
    mod_name: &str,
) -> Vec<String> {
    let mut names = Vec::new();
    let tmp: File = File::open(fpath).unwrap();
    let reader = BufReader::new(tmp);
    for l in reader.lines() {
        if let Ok(line) = l {
            if line.starts_with("message ") && line.ends_with("{") {
                let v: Vec<&str> = line.split(" ").collect();
                if v.len() != 3 {
                    panic!("wrong struct name: {}", line);
                }
                let target = v.get(1).unwrap();
                let t = target.trim().to_string();
                assert_eq!(unique.get(&t), None);
                unique.insert(t.clone(), mod_name.to_owned());
                names.push(t);
            }
        }
    }
    names
}

fn generate_allptos(
    w2fpath: &str,
    struct_names: &Vec<String>,
    _unique_names: &HashMap<String, String>,
    struct_names_in_mod: &HashMap<String, Vec<String>>,
    name2id: &HashMap<String, u32>,
) {
    let mut fh = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&w2fpath)
        .unwrap();

    let mut lines = vec![];
    for name in struct_names {
        let id = name2id.get(name).unwrap();
        let l = format!("{}=>{}", id, name);
        lines.push(l);
    }
    let contents = lines.join("\n");
    let hash = format!("{:x}", md5::compute(contents));
    let md5str = &hash[(hash.len() - 8)..];

    //文件头部
    let header = format!(
        "use ::prost::Message;
use std::{{result::Result}};

pub fn version() -> &'static str {{
    \"{md5str}\"
}}"
    );
    fh.write(header.as_bytes()).unwrap();
    fh.write("\n\n".as_bytes()).unwrap();

    //模块引用
    /*
    mod item;
    pub use item::Item;
         */
    // let mut modnames = vec![];
    // for (mod_name, _) in struct_names_in_mod {
    //     modnames.push(mod_name);
    // }
    // modnames.sort(); //保持有序
    // for mod_name in modnames {
    //     let structnames = struct_names_in_mod.get(mod_name).unwrap();
    //     let moder = format!("mod {mod_name};\n");
    //     fh.write(moder.as_bytes()).unwrap();
    // let allmods = structnames.join(",");
    // let str = if structnames.len() == 1 {
    //     format!("pub use {mod_name}::{allmods};\n")
    // } else {
    //     format!("pub use {mod_name}::{{{allmods}}};\n")
    // };
    // fh.write(str.as_bytes()).unwrap();
    // }
    // fh.write("\n\n".as_bytes()).unwrap();

    let mut modnames = vec![];
    for (mod_name, _) in struct_names_in_mod {
        modnames.push(mod_name);
    }
    modnames.sort(); //保持有序
    for mod_name in modnames {
        let structnames = struct_names_in_mod.get(mod_name).unwrap();

        // let moder = format!(
        //     "pub mod {mod_name} {{
        //     include!(\"protors/{mod_name}.rs\");
        // }}\n"
        // );
        //把所有文件写入进来，代替 include! 宏
        let file_path = format!("src/output/protors/{mod_name}.rs");
        let mut file = File::open(file_path).unwrap();
        let mut buffer = String::new();
        file.read_to_string(&mut buffer).unwrap();

        if mod_name == "embed" {
            let moder = format!("mod {mod_name} {{{buffer}}}\n");
            fh.write(moder.as_bytes()).unwrap();

            let allmods = structnames.join(",");
            let str = if structnames.len() == 1 {
                format!("pub use {mod_name}::{allmods};\n\n")
            } else {
                format!("pub use {mod_name}::{{{allmods}}};\n\n")
            };
            fh.write(str.as_bytes()).unwrap();
        } else {
            let moder = format!("{buffer}\n");
            fh.write(moder.as_bytes()).unwrap();
        }
    }
    fh.write("\n\n".as_bytes()).unwrap();

    //定义枚举结构
    /*
    #[derive(Debug)]
    pub enum ProtoType {
        Item(Item),
    }
         */
    let mut lines = vec![];
    for name in struct_names {
        let _id = name2id.get(name).unwrap();
        let l = format!("\t{name}({name}),");
        lines.push(l);
    }

    let pcontents = lines.join("\n");
    let pstr = format!(
        "#[derive(Debug)]
pub enum ProtoType {{
{pcontents}
}}"
    );
    fh.write(pstr.as_bytes()).unwrap();
    fh.write("\n\n".as_bytes()).unwrap();

    //实现枚举方法
    /*
    impl ProtoType {
        pub fn inner_info(&self) -> (u32,&'static str) {
            match self {
                ProtoType::Item(_obj) => { (101,"Item") },
            }
        }
    }
         */

    let mut lines = vec![];
    let mut from_id = vec![];
    let mut decode_from_lua = vec![];
    let mut encode_to_lua = vec![];
    let mut luaproto = vec![]; //导出 .lua 文件, return { [proto_id] = proto_name,...}
    luaproto.push("return {{".to_string());
    for name in struct_names {
        let id = name2id.get(name).unwrap();
        let l = format!("\t\t\tProtoType::{name}(_obj) => {{ ({id},\"{name}\") }},");
        lines.push(l);

        let s = format!(
            "{id} => {{
            let obj = {name}::default();
            Some(ProtoType::{name}(obj))
        }},"
        );
        from_id.push(s);

        let s = format!(
            "ProtoType::{name}(obj) => {{
            let obj = obj.from_lua_table(t)?;
            Ok(ProtoType::{name}(obj))
        }},"
        );
        decode_from_lua.push(s);

        let s = format!("ProtoType::{name}(obj) => obj.to_lua_table(ctx),");
        encode_to_lua.push(s);

        let s = format!("[{id}]=\"{name}\"");
        luaproto.push(s);
    }
    from_id.push("_=>{None},".to_owned());
    luaproto.push("}}".to_string());

    let pcontents = lines.join("\n");
    let pcontents_from_id = from_id.join("\n");
    let pcontents_decode_from_lua = decode_from_lua.join("\n");
    let pcontents_encode_to_lua = encode_to_lua.join("\n");
    let pstr = format!(
        "impl ProtoType {{
    pub fn inner_info(&self) -> (u32,&'static str) {{
        match self {{
{pcontents}
        }}
    }}

    pub fn from_id(proto_id: i32) -> Option<ProtoType> {{
        match proto_id {{
{pcontents_from_id}
        }}
    }}

    pub fn decode_from_lua(self, t: Table) -> rlua::Result<ProtoType> {{
        match self {{
    {pcontents_decode_from_lua}
        }}
    }}

    pub fn encode_to_lua(self, ctx: Context) -> rlua::Result<Table> {{
        match self {{
    {pcontents_encode_to_lua}
        }}
    }}
    
}}
"
    );
    fh.write(pstr.as_bytes()).unwrap();
    fh.write("\n\n".as_bytes()).unwrap();

    //decode函数
    let mut lines = vec![];
    for name in struct_names {
        let id = name2id.get(name).unwrap();
        let l = format!(
            "{id} => {{
    match {name}::decode(buf) {{
        Ok(obj) => Ok(ProtoType::{name}(obj)),
        Err(err) => Err(err.to_string()),
    }}
}},"
        );
        lines.push(l);
    }

    let pcontents = lines.join("\n");
    let pstr = format!(
        "pub fn decode(proto_id: u32,buf: &[u8]) -> Result<ProtoType,String> {{
    match proto_id {{
        {pcontents}
        _ => Err(format!(\"[decode]: failed=true, proto_id={{}}\",proto_id))
    }}
}}\n"
    );
    fh.write(pstr.as_bytes()).unwrap();

    //encode 函数
    let mut lines = vec![];
    for name in struct_names {
        let _id = name2id.get(name).unwrap();
        let l = format!(
            "ProtoType::{name}(obj) => {{
    let buff = obj.encode_to_vec();
    Ok(buff)
}},"
        );
        lines.push(l);
    }

    let pcontents = lines.join("\n");
    let pstr = format!(
        "pub fn encode(pto: ProtoType) -> Result<Vec<u8>,String> {{
    match pto {{
        {pcontents}
    }}
}}\n"
    );
    fh.write(pstr.as_bytes()).unwrap();

    fh.flush().unwrap();
}

fn generate_lua_encode_decode(
    w2fpath: &str,
    struct_names: &Vec<String>,
    _unique_names: &HashMap<String, String>,
    struct_names_in_mod: &HashMap<String, Vec<String>>,
    name2id: &HashMap<String, u32>,
) {
    let mut fh = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&w2fpath)
        .unwrap();

    //文件头部
    let header = format!("\n\nuse rlua::{{Context,Table}};");
    fh.write(header.as_bytes()).unwrap();
    fh.write("\n\n".as_bytes()).unwrap();

    let mut modnames = vec![];
    for (mod_name, _) in struct_names_in_mod {
        modnames.push(mod_name);
    }
    modnames.sort(); //保持有序
    for mod_name in modnames {
        //把所有文件写入进来，代替 include! 宏
        let file_path = format!("src/output/protors/{mod_name}.rs");
        // let mut filed_num = 0; //成员变量数量
        // {
        //     let tmp: File = File::open(file_path.clone()).unwrap();
        //     let reader = BufReader::new(tmp);
        //     for line in reader.lines() {
        //         let line = line.unwrap();
        //         let line = line.trim();
        //         if line.starts_with("#") || line.starts_with("//") {
        //             continue;
        //         }
        //         if !line.starts_with("pub struct") && line.starts_with("pub ") {
        //             filed_num += 1;
        //         }
        //     }
        // }
        let tmp: File = File::open(file_path).unwrap();
        let reader = BufReader::new(tmp);
        let mut to_block: Vec<String> = Vec::new();
        let mut from_block: Vec<String> = Vec::new();
        for line in reader.lines() {
            let line = line.unwrap();
            let line = line.trim();
            if line.starts_with("#") || line.starts_with("//") {
                continue;
            }
            if line == "}" {
                let s = "Ok(t)}\n\n".to_owned();
                to_block.push(s);

                let s = to_block.join("\n");
                fh.write(s.as_ref()).unwrap();
                to_block.clear();

                let s = "Ok(self)}".to_owned();
                from_block.push(s);

                let s = from_block.join("\n");
                fh.write(s.as_ref()).unwrap();
                from_block.clear();
            }
            // let line = format!("{line}\n");
            // fh.write(line.as_ref()).unwrap();
            /* 例子,
            impl CFeedback {
                pub fn to_lua_table(&self,ctx: Context) -> crate::Result<Table> {
                    let t: Table = ctx.create_table()?;
                    t.set("id",self.id);
                    t.set("msg",self.msg);
                    Ok(t)
                }
            }
            */
            if line.starts_with("pub struct") {
                // 以 pub struct xxx { 开头的行, 结构体声明
                assert!(line.contains("{"));
                let struct_name: Vec<&str> = line.split(" ").collect();
                let struct_name = struct_name.get(2).unwrap();
                if line.contains("}") {
                    //该协议无协议字段
                    let s = format!(
                        "impl {struct_name} {{
                        pub fn to_lua_table(self,ctx: Context) -> rlua::Result<Table> {{
                            let t: Table = ctx.create_table()?;
                            Ok(t)
                        }}"
                    );
                    to_block.push(s);
                    let s = to_block.join("\n");
                    fh.write(s.as_ref()).unwrap();
                    to_block.clear();

                    let s = format!(
                        "pub fn from_lua_table(self, _t: Table) -> rlua::Result<{struct_name}> {{
                        Ok(self)
                    }}"
                    );
                    from_block.push(s);
                    let s = from_block.join("\n");
                    fh.write(s.as_ref()).unwrap();
                    from_block.clear();
                } else {
                    let s = format!("impl {struct_name} {{");
                    fh.write(s.as_bytes()).unwrap();

                    let s = format!(
                        "pub fn to_lua_table(self,ctx: Context) -> rlua::Result<Table> {{
                    let t: Table = ctx.create_table()?;"
                    );
                    to_block.push(s);

                    let s = format!("pub fn from_lua_table(mut self, t: Table) -> rlua::Result<{struct_name}> {{");
                    from_block.push(s);
                }
            } else if line.starts_with("pub ") {
                // 以 pub xx: 开头的行, 结构体成员变量
                //pub id: u32,
                //pub msg: ::prost::alloc::string::String,
                //pub items: ::prost::alloc::vec::Vec<embed::Item>,
                //pub items: ::core::option::Option<embed::Item>,
                let s: Vec<&str> = line.split(" ").collect();
                let member_name = s.get(1).unwrap();
                let member_name = &member_name[..member_name.len() - 1]; //移除:号
                let member_type = s.get(2).unwrap();
                let member_type = &member_type[..&member_type.len() - 1]; //移除,号

                //先移除所有的protobuf类型的前缀, 例如 ::prost::alloc::string:: , ::prost::alloc::vec:: ,::core::option:: 等等
                let member_type = member_type.replace("::prost::alloc::string::", "");
                let member_type = member_type.replace("::prost::alloc::vec::", "");
                let mut member_type = member_type.replace("::core::option::", "");
                //类型信息剩下 u32,String,Vec<xxx>,Option<xxx>
                if member_type.starts_with("Vec<") && member_type.ends_with(">") {
                    //Vec<embed::Item>,Vec<String>,Vec<u32>
                    //数组
                    //剥离 Vec< 和 >
                    let mut member_type = member_type.replace("Vec<", "").replace(">", "");

                    //嵌套类型,例如 embed::Item
                    if let Some(pos) = member_type.rfind(":") {
                        member_type.replace_range(..(pos + 1), "");
                        //Item
                        if name2id.contains_key(&member_type) {
                            let s = format!(
                                "
                            let t_{member_name}: Table = ctx.create_table()?;
                            let mut index = 0;
                            for val in self.{member_name} {{
                                let tt: Table = val.to_lua_table(ctx)?;
                                t_{member_name}.raw_set(index,tt)?;
                                index +=1;
                            }}
                            t.raw_set(\"{member_name}\",t_{member_name})?;"
                            );
                            to_block.push(s);

                            let s = format!("let t_{member_name}: Table = t.raw_get(\"{member_name}\")?;
                            let len = t_{member_name}.len()?;
                            if len >= 1000 {{
                                return Err(rlua::Error::RuntimeError(
                                    \"{member_name} table len limit\".to_owned(),
                                ));
                            }}
                            let mut {member_name} = Vec::<{member_type}>::with_capacity(len as usize);
                            for index in 0..len {{
                                let tt: Table = t_{member_name}.raw_get(index)?;
                                let item = {member_type}::default();
                                let item = item.from_lua_table(tt)?;
                                {member_name}.push(item);
                            }}
                            self.{member_name} = {member_name};");
                            from_block.push(s);
                        } else {
                            assert!(false); //error
                        }
                    } else {
                        //原生类型, 例如 String, u32 之类的
                        let s = format!(
                            "
                        let t_{member_name}: Table = ctx.create_table()?;
                        let mut index = 0;
                        for val in self.{member_name} {{
                            t_{member_name}.raw_set(index,val)?;
                            index +=1;
                        }}
                        t.raw_set(\"{member_name}\",t_{member_name})?;"
                        );
                        to_block.push(s);

                        let s = format!(
                            "let {member_name}: Table = t.raw_get(\"{member_name}\")?;
                        let len = {member_name}.len()?;
                        for index in 0..len {{
                            let t: {member_type} = {member_name}.raw_get(index)?;
                            self.{member_name}.push(t);
                        }}"
                        );
                        from_block.push(s);
                    }
                } else if member_type.starts_with("Option<") && member_type.ends_with(">") {
                    //剥离 Option< 和 >
                    let mut member_type = member_type.replace("Option<", "").replace(">", "");
                    //嵌套类型,例如 embed::Item
                    if let Some(pos) = member_type.rfind(":") {
                        member_type.replace_range(..(pos + 1), "");
                        //Item
                        if name2id.contains_key(&member_type) {
                            let s = format!(
                                "if let Some(val) = self.{member_name} {{
                                let tt: Table = val.to_lua_table(ctx)?;
                                t.raw_set(\"{member_name}\",tt)?;
                            }}"
                            );
                            to_block.push(s);

                            let s = format!(
                                "if t.contains_key(\"{member_name}\") {{
                                let tt: Table = t.raw_get(\"{member_name}\")?;
                                let item = {member_type}::default();
                                let item = item.from_lua_table(tt)?;
                                self.{member_name} = Some(item);
                            }} else {{
                                self.{member_name} = None;
                            }}"
                            );
                            from_block.push(s);
                        } else {
                            assert!(false); //error
                        }
                    } else {
                        //原生类型, 例如 String, u32 之类的
                        let s = format!(
                            "if let Some(val) = self.{member_name} {{
                            t.raw_set(\"{member_name}\",val)?;
                        }}"
                        );
                        to_block.push(s);

                        let s = format!(
                            "if t.contains_key(\"{member_name}\") {{
                            let tt: {member_type} = t.raw_get(\"{member_name}\")?;
                            self.{member_name} = Some(tt);
                        }} else {{
                            self.{member_name} = None;
                        }}"
                        );
                        from_block.push(s);
                    }
                } else {
                    //嵌套类型,例如 embed::Item
                    if let Some(pos) = member_type.rfind(":") {
                        member_type.replace_range(..(pos + 1), "");
                        //嵌套类型,例如 embed::Item
                        if name2id.contains_key(&member_type) {
                            // embed
                            let s = format!(
                                "let tt: Table = self.{member_name}.to_lua_table(ctx)?;
                                t.raw_set(\"{member_name}\",tt)?;"
                            );
                            to_block.push(s);

                            let s = format!(
                                "let tt: Table = t.raw_get(\"{member_name}\")?;
                            self.{member_name} = self.{member_name}.from_lua_table(tt)?;"
                            );
                            from_block.push(s);
                        } else {
                            assert!(false); //error
                        }
                    } else {
                        //原生类型, 例如 String, u32 之类的
                        let s = format!("t.raw_set(\"{member_name}\",self.{member_name})?;");
                        to_block.push(s);

                        let s = format!(
                            "let tt: {member_type} = t.raw_get(\"{member_name}\")?;
                        self.{member_name} = tt;"
                        );
                        from_block.push(s);
                    }
                };
            }
        }
        let s = to_block.join("\n");
        fh.write(s.as_ref()).unwrap();
        fh.write("}\n\n".as_bytes()).unwrap();
    }
    fh.write("\n\n".as_bytes()).unwrap();
}

fn generate_allprotos_lua(
    w2fpath: &str,
    struct_names: &Vec<String>,
    name2id: &HashMap<String, u32>,
) {
    let mut fh = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&w2fpath)
        .unwrap();

    let mut luaproto = vec![]; //导出 .lua 文件, return { [proto_id] = proto_name,...}
    luaproto.push("return {".to_string());
    for name in struct_names {
        let id = name2id.get(name).unwrap();
        let s = format!("\t[{id}]=\"{name}\",");
        luaproto.push(s);
    }
    luaproto.push("}".to_string());
    let pcontents = luaproto.join("\n");
    fh.write(pcontents.as_bytes()).unwrap();
    fh.write("\n\n".as_bytes()).unwrap();
}

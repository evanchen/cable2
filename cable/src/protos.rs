pub use protogen::output::allprotos::*;
use rlua::{Context, Table, Value};
use std::io::Write;

pub fn serialize_table_to_string(ctx: Context, t: Table) -> rlua::Result<Vec<u8>> {
    let s = Vec::<u8>::with_capacity(1024);
    let depth = 0;
    table_to_string(ctx, t, s, depth)
}

fn table_to_string(ctx: Context, t: Table, mut s: Vec<u8>, depth: i32) -> rlua::Result<Vec<u8>> {
    s.push(b'{');
    if depth >= 20 {
        return Err(rlua::Error::RuntimeError("table too depth".to_string()));
    }
    for pairs in t.pairs::<Value, Value>() {
        let (key, value) = pairs?;
        s = to_key(ctx, key, s)?;
        s.push(b'=');
        s = to_value(ctx, value, s, depth + 1)?;
        s.push(b',');
    }
    s.push(b'}');
    Ok(s)
}

fn to_key(ctx: Context, key: Value, mut s: Vec<u8>) -> rlua::Result<Vec<u8>> {
    match key {
        Value::Integer(i) => {
            if let Err(err) = write!(&mut s, "[{}]", i) {
                return Err(rlua::Error::RuntimeError(err.to_string()));
            }
            Ok(s)
        }
        Value::Number(n) => {
            if let Err(err) = write!(&mut s, "[{}]", n) {
                return Err(rlua::Error::RuntimeError(err.to_string()));
            }
            Ok(s)
        }
        Value::String(_) => {
            s.push(b'[');
            s = to_string(ctx, key, s)?;
            s.push(b']');
            Ok(s)
        }
        _ => {
            let err = format!("[to_key]:unspport key type '{}'", key.type_name());
            Err(rlua::Error::RuntimeError(err))
        }
    }
}

fn to_value(ctx: Context, value: Value, mut s: Vec<u8>, depth: i32) -> rlua::Result<Vec<u8>> {
    match value {
        Value::Integer(i) => {
            if let Err(err) = write!(&mut s, "{}", i) {
                return Err(rlua::Error::RuntimeError(err.to_string()));
            }
            Ok(s)
        }
        Value::Number(n) => {
            if let Err(err) = write!(&mut s, "{}", n) {
                return Err(rlua::Error::RuntimeError(err.to_string()));
            }
            Ok(s)
        }
        Value::String(_) => to_string(ctx, value, s),
        Value::Boolean(b) => {
            if let Err(err) = write!(&mut s, "{}", b) {
                return Err(rlua::Error::RuntimeError(err.to_string()));
            }
            Ok(s)
        }
        Value::Table(t) => table_to_string(ctx, t, s, depth),
        Value::Nil => {
            if let Err(err) = write!(&mut s, "nil") {
                return Err(rlua::Error::RuntimeError(err.to_string()));
            }
            Ok(s)
        }
        _ => {
            let err = format!(
                "[to_value]:unspport value type '{}',{:?}",
                value.type_name(),
                value
            );
            Err(rlua::Error::RuntimeError(err))
        }
    }
}

//参考lua的字符串,应该用 string.format("%q",str) 来还原(lua源码实现: lstrlib.c: addquoted()  )
//这里做简化处理
/*
static void addquoted (luaL_Buffer *b, const char *s, size_t len) {
  luaL_addchar(b, '"');
  while (len--) {
    if (*s == '"' || *s == '\\' || *s == '\n') {
      luaL_addchar(b, '\\');
      luaL_addchar(b, *s);
    }
    else if (iscntrl(uchar(*s))) {
      char buff[10];
      if (!isdigit(uchar(*(s+1))))
        l_sprintf(buff, sizeof(buff), "\\%d", (int)uchar(*s));
      else
        l_sprintf(buff, sizeof(buff), "\\%03d", (int)uchar(*s));
      luaL_addstring(b, buff);
    }
    else
      luaL_addchar(b, *s);
    s++;
  }
  luaL_addchar(b, '"');
}
*/
fn to_string(_ctx: Context, value: Value, mut s: Vec<u8>) -> rlua::Result<Vec<u8>> {
    if let Value::String(ss) = value {
        match ss.to_str() {
            Ok(sss) => {
                s.push(b'\"');
                //必须是 utf-8 格式的字符串
                for c in sss.as_bytes() {
                    match c {
                        b'"' | b'\\' | b'\n' => {
                            s.push(b'\\');
                            s.push(*c);
                        }
                        b'\r' => {
                            let mut tmp: Vec<u8> = Vec::with_capacity(10);
                            if let Err(err) = write!(&mut tmp, "\\r") {
                                return Err(rlua::Error::RuntimeError(err.to_string()));
                            }
                            s.append(&mut tmp);
                        }
                        b'\0' => {
                            let mut tmp: Vec<u8> = Vec::with_capacity(10);
                            if let Err(err) = write!(&mut tmp, "\\000") {
                                return Err(rlua::Error::RuntimeError(err.to_string()));
                            }
                            s.append(&mut tmp);
                        }
                        _ => {
                            s.push(*c);
                        }
                    }
                }
                s.push(b'\"');
                Ok(s)
            }
            Err(err) => {
                let err = format!("[to_string]:not a utf-8 string {}", err);
                return Err(rlua::Error::RuntimeError(err));
            }
        }
    } else {
        let err = format!("[to_string]:unspport string '{:?}'", value);
        return Err(rlua::Error::RuntimeError(err));
    }
}

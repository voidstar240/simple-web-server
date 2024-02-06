use std::{path::PathBuf, io::Read, ops::Deref};
use http_body_util::{Full, BodyExt};
use hyper::{Request, Response, body::Bytes};
use rlua::Lua;

pub enum ResponseType {
    Resource(ResourceResponse),
    Script(ScriptResponse),
}

impl ResponseType {
    pub async fn respond(&self, request: Request<hyper::body::Incoming>) -> Result<Response<Full<Bytes>>, String> {
        match self {
            Self::Script(s) => s.respond(request).await,
            Self::Resource(r) => r.respond(request).await, 
        }
    }
}

pub struct ResourceResponse {
    pub path: PathBuf,
    pub status_code: u16,
    pub headers: Vec<(String, String)>,
}

impl ResourceResponse {
    async fn respond(&self, _request: Request<hyper::body::Incoming>) -> Result<Response<Full<Bytes>>, String> {
        if let Ok(mut file) = std::fs::File::open(&self.path) {
            let mut bytes = vec![];
            if let Ok(_) = file.read_to_end(&mut bytes) {
                let mut resp = Response::builder().status(self.status_code);
                for (header, val) in &self.headers {
                    resp = resp.header(header, val);
                }
                return resp.body(Full::new(Bytes::from(bytes))).map_err(|e| e.to_string());
            }
            return Err(format!("Could not read file at {}", self.path.to_string_lossy()));
        }
        Err(format!("Could not open file at {}", self.path.to_string_lossy()))
    }
}

pub struct ScriptResponse {
    pub path: PathBuf,
}

impl ScriptResponse {
    async fn respond(&self, request: Request<hyper::body::Incoming>) -> Result<Response<Full<Bytes>>, String> {
        let code = std::fs::read_to_string(&self.path).map_err(|_| format!("could not open/read {}", self.path.to_string_lossy()))?;
        println!("Executing Script: {}", self.path.to_string_lossy());

        let method: String = request.method().to_string();
        let headers: Vec<(String, String)> = request.headers().iter().filter_map(|(k, v)| v.to_str().ok().map(|v_str| (k.to_string(), v_str.to_string()))).collect();
        let path: String = request.uri().path().to_string();
        let query: String = request.uri().query().unwrap_or("").to_string();
        let body = unsafe { std::str::from_utf8_unchecked(request.into_body().collect().await.unwrap().to_bytes().deref()).to_string() };

        let lua: Lua = Lua::new();
        let lua_resp: LuaResponse = lua.context(|lua_ctx| {
            let header_table = lua_ctx.create_table().unwrap();
            for (k, v) in headers {
                header_table.set(k, v).unwrap();
            }

            let req_table = lua_ctx.create_table().unwrap();
            req_table.set("method", method).unwrap();
            req_table.set("headers", header_table).unwrap();
            req_table.set("path", path).unwrap();
            req_table.set("query", query).unwrap();
            req_table.set("body", body).unwrap();

            lua_ctx.load(&code).call(req_table).unwrap()
        });
        let mut resp_bldr = Response::builder().status(lua_resp.status_code);
        for (k, v) in lua_resp.headers.iter() {
            resp_bldr = resp_bldr.header(k, v);
        }

        Ok(resp_bldr
            .body(Full::new(Bytes::from(lua_resp.body)))
            .unwrap())
    }
}

struct LuaResponse {
    status_code: u16,
    body: Vec<u8>,
    headers: Vec<(String, String)>
}

impl<'lua> rlua::FromLua<'lua> for LuaResponse {
    fn from_lua(lua_value: rlua::Value<'lua>, _lua: rlua::Context<'lua>) -> rlua::Result<Self> {
        return match lua_value {
            rlua::Value::Table(t) => {
                let status_code: u16 = match t.get("status_code") {
                    Ok(v) =>  v,
                    Err(_) => return Err(rlua::Error::FromLuaConversionError { from: "Response", to: "LuaResponse", message: Some(String::from("Table key 'status_code' must be a u16.")) }),
                };

                let bytes = match t.get("body") {
                    Ok(v) => v,
                    Err(_) => return Err(rlua::Error::FromLuaConversionError { from: "Response", to: "LuaResponse", message: Some(String::from("Table key 'body' must be able to be converted to a Vec<u8>.")) }),
                };

                let mut headers = vec![];
                match t.get::<&'static str, rlua::Table>("headers") {
                    Ok(head_t) => for (k, v) in head_t.pairs::<String, rlua::Value>().filter_map(|x| x.ok()) {
                        if let rlua::Value::String(s) = v {
                            if let Ok(s) = s.to_str() {
                                headers.push((k, String::from(s)));
                            }
                        }
                    },
                    Err(_) => return Err(rlua::Error::FromLuaConversionError { from: "Response", to: "LuaResponse", message: Some(String::from("Table key 'headers' must be a Table")) }),
                };

                Ok(Self {
                    status_code,
                    body: bytes,
                    headers,
                })
            },
            rlua::Value::String(s) => Ok(Self {
                status_code: 200,
                body: s.as_bytes().to_vec(),
                headers: vec![],
            }),
            _ => Err(rlua::Error::FromLuaConversionError { from: "Response", to: "LuaResponse", message: Some(String::from("Response must be a Table or String")) }),
        }
    }
}

pub fn empty_response() -> ResponseType {
    ResponseType::Script(ScriptResponse { path: PathBuf::from("") })
}

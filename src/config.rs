use hyper::Method;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;

use crate::response::*;


pub struct ServerConfig {
    pub port: u16,
    pub public_web_root: PathBuf,
    pub home_page_response: ResponseType,
    pub not_found_response: ResponseType,
    pub custom_urls: HashMap<(Method, String), ResponseType>
}


pub fn parse_json_config(json: &Value) -> Result<ServerConfig, String> {
    let mut errors = String::from("");
    let port: u16 = match &json["port"] {
        Value::Number(n) => {
            if let Some(port_u64) = n.as_u64() {
                if let Ok(port) = port_u64.try_into() {
                    port
                } else {
                    errors.push_str("\"port\" must be an unsigned int <65536\n");
                    0
                }
            } else {
                errors.push_str("\"port\" must be an unsigned int <65536\n");
                0
            }
        }
        Value::Null => { errors.push_str("\"port\" not found\n"); 0 },
        _ => { errors.push_str("\"port\" must be an unsigned int <65536\n"); 0 },
    };

    let public_web_root = match &json["public-web-root"] {
        Value::String(s) => PathBuf::from(s),
        Value::Null => { errors.push_str("\"public-web-root\" not found\n"); PathBuf::from("") },
        _ => { errors.push_str("\"public-web-root\" must be a string\n"); PathBuf::from("") },
    };

    let home_page_response = match parse_json_response(&json["home-page-response"]) {
        Ok(resp) => resp,
        Err(e) => { errors.push_str(&format!("\"home-page-response\" errors:\n{}\n", e)); empty_response() },
    };

    let not_found_response = match parse_json_response(&json["not-found-response"]) {
        Ok(resp) => resp,
        Err(e) => { errors.push_str(&format!("\"not-found-response\" errors:\n{}\n", e)); empty_response() },
    };

    let mut custom_urls = HashMap::new();
    match &json["custom-urls"] {
        Value::Array(v) => for (index, route) in v.iter().enumerate() {
            let mut route_errors = String::from("");
            let method = match &route["method"] {
                Value::String(s) => Method::from_str(s).map_err(
                    |_| format!("\"{}\" is not a valid request method\n", s))?,
                Value::Null => Method::GET,
                _ => { route_errors.push_str("\"method\" must be a string\n"); Method::GET },
            };

            let web_path = match &route["web-path"] {
                Value::String(s) => s.clone(),
                Value::Null => { route_errors.push_str("\"web-path\" must be provided\n"); String::from("") },
                _ => { route_errors.push_str("\"web-path\" must be a string\n"); String::from("") },
            };

            let response = match parse_json_response(&route["response"]) {
                Ok(resp) => resp,
                Err(e) => { route_errors.push_str(&format!("\"custom-urls\"[{}].\"response\" errors:\n{}", index, e)); empty_response() }
            };

            if route_errors.len() == 0 {
                custom_urls.insert((method, web_path), response);
            } else {
                errors.push_str(&format!("\"custom-urls\"[{}] errors:\n{}\n", index, route_errors));
            }
        },
        Value::Null => {},
        _ => errors.push_str("\"custom-urls\" must be an array\n"),
    }

    if errors.len() == 0 {
        Ok(ServerConfig {
            port,
            public_web_root,
            home_page_response,
            not_found_response,
            custom_urls,
        })
    } else {
        Err(errors)
    }
}

pub fn parse_json_response(json: &Value) -> Result<ResponseType, String> {
    if !json.is_object() {
        return Err(String::from("response must be an object\n"));
    }

    if json["script"] != Value::Null {
        // is a script
        return Ok(ResponseType::Script(parse_json_script_response(json)?));
    }

    if json["resource-location"] != Value::Null {
        // is a resource
        return Ok(ResponseType::Resource(parse_json_resource_response(json)?));
    }

    Err(String::from("no \"script\" or \"resource-location\" field\n"))
}

pub fn parse_json_resource_response(json: &Value) -> Result<ResourceResponse, String> {
    let mut errors = String::from("");
    let resource = match &json["resource-location"] {
        Value::String(s) => PathBuf::from_str(s).unwrap(),
        _ => { errors.push_str("\"resource-location\" must be a string\n"); PathBuf::from("") },
    };

    let status_code: u16 = match &json["status-code"] {
        Value::Number(n) => {
            if let Some(code_u64) = n.as_u64() {
                if let Ok(code) = code_u64.try_into() {
                    code
                } else {
                    errors.push_str("\"status-code\" must be an unsigned int <65536\n");
                    0
                }
            } else {
                errors.push_str("\"status-code\" must be an unsigned int <65536\n");
                0
            }
        },
        Value::Null => 200,
        _ => { errors.push_str("\"status-code\" must be an unsigned int <65536\n"); 0 },
    };

    let mut headers: Vec<(String, String)> = vec![];
    match &json["headers"] {
        Value::Object(o) => {
            for (k, v) in o.iter() {
                let v_str = match v {
                    Value::String(s) => s.clone(),
                    Value::Null => String::from(""),
                    _ => v.to_string(),
                };
                headers.push((k.clone(), v_str));
            }
        },
        Value::Null => {},
        _ => errors.push_str("\"headers\" must be an object\n"),
    }

    if errors.len() == 0 {
        Ok(ResourceResponse {
            path: resource,
            status_code,
            headers,
        })
    } else {
        Err(errors)
    }
}

pub fn parse_json_script_response(json: &Value) -> Result<ScriptResponse, String> {
    let script = match &json["script"] {
        Value::String(s) => PathBuf::from_str(s).unwrap(),
        _ => return Err(String::from("\"script\" must be a string\n")),
    };

    Ok(ScriptResponse {
        path: script,
    })
}

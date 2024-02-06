use std::convert::Infallible;
use std::io::Read;
use std::net::{SocketAddr, IpAddr};
use std::path::PathBuf;
use serde_json::Value;
use tokio::net::TcpListener;
use hyper_util::rt::TokioIo;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, Method};
use hyper::body::Bytes;
use http_body_util::Full;

mod config;
mod response;
use config::*;

static mut CONFIG: Option<ServerConfig> = None;

fn config() -> &'static ServerConfig {
    unsafe { CONFIG.as_ref().expect("Global CONFIG is uninitialized") }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut file = std::fs::File::open("server_config.json").expect("Unable to open file \"server_config.json\"");
    let mut str_data = String::new();
    file.read_to_string(&mut str_data).expect("Unable to read \"server_config.json\"");
    let config_json: Value = json5::from_str(&mut str_data).expect("Unable to parse \"server_config.json\"");


    unsafe { CONFIG = Some(match parse_json_config(&config_json) {
        Ok(conf) => conf,
        Err(e) => panic!("\nEncountered errors while parsing server_config.json:\n{}\n", e),
    }); }

    println!("Server Started");

    let addr = SocketAddr::from(([127, 0, 0, 1], config().port));

    let listener = TcpListener::bind(addr).await?;

    println!("Listening for incoming connectons at {} ...", listener.local_addr().expect("Server hosted on invalid address"));


    // We start a loop to continuously accept incoming connections
    loop {
        let (stream, _) = listener.accept().await?;

        // Use an adapter to access something implementing `tokio::io` traits as if they implement
        // `hyper::rt` IO traits.
        let io = TokioIo::new(stream);

        // Spawn a tokio task to serve multiple connections concurrently
        tokio::task::spawn(async move {
            // Finally, we bind the incoming connection to our `hello` service
            let addr = io.inner().peer_addr().expect("Invalid peer address").ip();
            if let Err(err) = http1::Builder::new()
                // `service_fn` converts our function in a `Service`
                .serve_connection(io, service_fn(|req| request_handler(req, addr)))
                .await
            {
                println!("Error serving connection: {:?}", err);
            }
        });
    }
}

async fn request_handler(request: Request<hyper::body::Incoming>, ip_addr: IpAddr) -> Result<Response<Full<Bytes>>, Infallible> {
    println!("Serving connection from {}, Request version: {:?}, URI: {}", ip_addr, request.version(), request.uri());
    let path = request.uri().path().to_owned();

    // is the request trying to access the root page
    if (path == "/" || path == "") && request.method() == Method::GET {
        match config().home_page_response.respond(request).await {
            Ok(response) => return Ok(response),
            Err(e) => {
                println!("Error: {}", e);
                return Ok(error_response("The server has encountered an internal error."));
            },
        }
    }

    // try the request against the custom urls
    if let Some(response_type) = config().custom_urls.get(&(request.method().clone(), path.clone())) {
        match response_type.respond(request).await {
            Ok(response) => {
                return Ok(response);
            },

            Err(e) => {
                println!("Error: {}", e);
                return Ok(error_response("The server has encountered an internal error."));
            },
        }
    }

    // fallback on browsing the public root directly
    let native_path = PathBuf::from(path.trim_start_matches("/"));
    let mut new_path: PathBuf = config().public_web_root.clone();
    new_path.push(native_path);
    if request.method() == Method::GET {
        if let Ok(mut file) = std::fs::File::open(new_path) {
            let mut bytes = vec![];
            if let Ok(_) = file.read_to_end(&mut bytes) {
                return Ok(
                    Response::builder()
                        .status(200)
                        // TODO extract media type from file extension
                        // instead of using MIME sniffing
                        //.header(hyper::header::CONTENT_TYPE, "MEDIA TYPE")
                        .body(Full::new(Bytes::from(bytes)))
                        .unwrap()
                );
            }
        }
    }

    // 404 if all else failed
    match config().not_found_response.respond(request).await {
        Ok(response) => return Ok(response),
        Err(e) => {
            println!("Error: {}", e);
            return Ok(error_response("The server has encountered an internal error."));
        },
    }
}

fn error_response<T: ToString>(msg: T) -> Response<Full<Bytes>> {
    Response::builder()
        .status(500)
        .header(hyper::header::CONTENT_TYPE, "")
        .body(Full::new(Bytes::from(msg.to_string())))
        .unwrap()
}

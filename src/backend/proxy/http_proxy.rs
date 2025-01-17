#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(non_camel_case_types)]

use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncRead, AsyncWrite, AsyncReadExt, AsyncWriteExt};
use time::OffsetDateTime;
use colored::*;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use structopt::StructOpt;
use tokio::time::Duration;
use tokio::time::sleep;
use crate::{info,error,debug};
use rustls::{ServerConfig, ClientConfig};
use rcgen::{Certificate, CertificateParams, DistinguishedName, DnType, SanType};
use rustls_pemfile::{certs, pkcs8_private_keys};
use std::fs::File;
use std::io::Write;


#[derive(Debug, StructOpt)]
#[structopt(name = "http_proxy", about = "A simple HTTP proxy tool")]
pub struct ProxyConfig {
    #[structopt(short = "l", long = "listen", default_value = "127.0.0.1:8081")]
    listen_addr: String,
    
    #[structopt(short = "t", long = "target", default_value = "127.0.0.1:7890")]
    target_addr: String,
}


static REQUEST_COUNTER: AtomicU32 = AtomicU32::new(1);

#[derive(Debug, Serialize, Deserialize)]
pub struct HttpRequest {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    #[serde(default)]  
    body: Vec<u8>,
}


/*

    - 监听地址: 127.0.0.1:8081  (proxy server)
    - 目标地址: 127.0.0.1:7890  (target server)

    当浏览器(client)访问 127.0.0.1:8081 时:
    1. 浏览器 -> 代理服务器   (产生client_stream)
    2. 代理服务器 -> 目标服务器 (产生target_stream)

    数据流向：
    浏览器 <-> client_stream <-> 代理服务器 <-> target_stream <-> 目标服务器

*/

impl HttpRequest {
    // The raw data is parsed to HTTP
    async fn from_raw_data(data: &[u8]) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let request_str = String::from_utf8_lossy(data);
        let mut headers = Vec::new();
        let mut body = Vec::new();
        let mut method = String::new();
        let mut path = String::new();
        
        let mut lines = request_str.lines();
        

        // 解析请求行
        if let Some(first_line) = lines.next() {
            let parts: Vec<&str> = first_line.split_whitespace().collect();
            if parts.len() >= 2 {
                method = parts[0].to_string();
                path = parts[1].to_string();
            }
        }
        // 解析头部和主体
        let mut is_body = false;
        let mut body_str = String::new();
        for line in lines {
            if line.is_empty() {
                is_body = true;
                continue;
            }
            if !is_body {
                if let Some(idx) = line.find(':') {
                    let (key, value) = line.split_at(idx);
                    headers.push((
                        key.trim().to_string(),
                        value[1..].trim().to_string(),
                    ));
                }
            } else {
                body_str.push_str(line);
                body_str.push('\n');
            }
        }
        // 处理主体
        body = body_str.into_bytes();
        Ok(HttpRequest {
            method,
            path,
            headers,
            body,
        })

    }
}


// HTTP log requests
async fn logger_request(httpreq: &HttpRequest, source: &str, target: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S.%3f").to_string();

    // 连接信息
    info!("{} [DEBUG] >>> New connection from {}", timestamp, source);
    info!("{} [DEBUG] >>> Connected to target at {}", timestamp, target);
    
    // 完整请求信息作为一个整体
    let mut request_content = String::new();
    request_content.push_str(&format!("Read {} bytes from client\n", httpreq.body.len()));
    request_content.push_str(&format!("{} {} HTTP/1.1\n", httpreq.method, httpreq.path));
    
    // 请求头
    for (name, value) in &httpreq.headers {
        request_content.push_str(&format!("{}: {}\n", name, value));
    }

    // 请求体
    if !httpreq.body.is_empty() {
        request_content.push_str("\n"); // 空行分隔header和body
        match String::from_utf8(httpreq.body.clone()) {
            Ok(body_str) => {
                match serde_json::from_str::<serde_json::Value>(&body_str) {
                    Ok(json) => {
                        if let Ok(formatted_json) = serde_json::to_string_pretty(&json) {
                            request_content.push_str(&formatted_json);
                            request_content.push_str("\n");
                        }
                    },
                    Err(_) => {
                        request_content.push_str(&body_str);
                        request_content.push_str("\n");
                    }
                }
            },
            Err(_) => request_content.push_str("[Binary content]\n"),
        }
    }

    info!("{}", request_content);
    Ok(())
}


// Data is forwarded in both directions
async fn copy_bidirectional(
    client: &mut TcpStream,
    target: &mut TcpStream,
    client_addr: String,
    target_addr: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (mut client_read, mut client_write) = client.split();
    let (mut target_read, mut target_write) = target.split();
    let response_content = Arc::new(Mutex::new(String::new()));
    let response_content_clone = response_content.clone();
    let client_to_target = async {
        let mut buffer = vec![0; 65536];
        loop {
            let n = match client_read.read(&mut buffer).await {
                Ok(0) => {
                    //debug!("Client closed connection");
                    break Ok(());
                },
                Ok(n) => {
                    //debug!("Read {} bytes from client", n);
                    n
                },
                Err(e) => {
                    //debug!("Error reading from client: {}", e);
                    return Err(Box::new(e) as Box<dyn std::error::Error + Send + Sync>);
                }
            };

            if let Ok(req) = HttpRequest::from_raw_data(&buffer[..n]).await {
                if let Err(e) = logger_request(&req, &client_addr, target_addr).await {
                    //debug!("Error logging request: {}", e);
                }
            }

            match target_write.write_all(&buffer[..n]).await {
                Ok(_) => {
                    //debug!("Sent {} bytes to target", n);
                    if let Err(e) = target_write.flush().await {
                        //debug!("Error flushing to target: {}", e);
                        return Err(Box::new(e));
                    }
                },
                Err(e) => {
                    //debug!("Error writing to target: {}", e);
                    return Err(Box::new(e));
                }
            }
        }
    };

    let target_to_client = async {
        let mut buffer = vec![0; 65536];
        loop {
            let n = match target_read.read(&mut buffer).await {
                Ok(0) => {
                    //debug!("Target closed connection");
                    break Ok(());
                },
                Ok(n) => {
                    //debug!("Read {} bytes from target", n);
                    n
                },
                Err(e) => {
                    //debug!("Error reading from target: {}", e);
                    return Err(Box::new(e) as Box<dyn std::error::Error + Send + Sync>);
                }
            };

            //debug!("Response from target: {} bytes", n);
            if let Ok(resp_str) = String::from_utf8(buffer[..n].to_vec()) {
               // debug!("Response Content:\n {}",resp_str);
               let mut content = response_content_clone.lock().await;
               content.push_str(&resp_str);
            }
            match client_write.write_all(&buffer[..n]).await {
                Ok(_) => {
                   // debug!("Sent {} bytes to client", n);
                    if let Err(e) = client_write.flush().await {
                        //debug!("Error flushing to client: {}", e);
                        return Err(Box::new(e));
                    }
                },
                Err(e) => {
                    //debug!("Error writing to client: {}", e);
                    return Err(Box::new(e));
                }
            }
        }
    };
    tokio::select! {
        result1 = client_to_target => {
            debug!("Client to target finished: {:?}", result1);
            result1?;
        },
        result2 = target_to_client => {
            debug!("Target to client finished: {:?}", result2);
            result2?;
        }
    }
    let content = response_content.lock().await;
    debug!("Response Content:\n{}",content.as_str());
    Ok(())
}







#[derive(Debug, Clone, PartialEq)]
pub enum ProxyProtocol {
    HTTP,
    HTTPS,
    Unknown,
}

async fn detect_protocol(stream: &mut TcpStream) -> Result<ProxyProtocol, Box<dyn std::error::Error + Send + Sync>> {
    let mut peek_buf = vec![0u8; 1024];  // 增加缓冲区大小以获取更多信息
    let n = stream.peek(&mut peek_buf).await?;
    
    if n == 0 {
        return Ok(ProxyProtocol::Unknown);
    }

    // 检查是否为 TLS 握手
    if peek_buf[0] == 0x16 && n >= 3 {
        // TLS 版本检查 (0x03 0x01 for TLS 1.0, 0x03 0x03 for TLS 1.2)
        if peek_buf[1] == 0x03 && (peek_buf[2] == 0x01 || peek_buf[2] == 0x02 || peek_buf[2] == 0x03) {
            debug!("Detected TLS handshake");
            return Ok(ProxyProtocol::HTTPS);
        }
    }

    // 尝试解析 HTTP 请求
    let request_str = String::from_utf8_lossy(&peek_buf[..n]);
    let first_line = request_str.lines().next().unwrap_or("");
    
    debug!("First line of request: {}", first_line);

    // 检查是否为 HTTP CONNECT 方法（通常用于HTTPS隧道）
    if first_line.starts_with("CONNECT") {
        debug!("Detected HTTPS (CONNECT method)");
        return Ok(ProxyProtocol::HTTPS);
    }

    // 检查其他 HTTP 方法
    if first_line.contains(" HTTP/1.") || first_line.contains(" HTTP/2.") {
        let parts: Vec<&str> = first_line.split_whitespace().collect();
        if parts.len() >= 3 {
            let method = parts[0];
            let path = parts[1];
            
            debug!("HTTP Method: {}, Path: {}", method, path);
            
            // 如果路径是 HTTPS URL，可能是 HTTPS 请求
            if path.starts_with("https://") {
                return Ok(ProxyProtocol::HTTPS);
            }
            
            return Ok(ProxyProtocol::HTTP);
        }
    }

    // 检查常见的 TLS/SSL 特征
    let is_possibly_tls = peek_buf.windows(3).any(|window| {
        // SSLv3, TLS 1.0, 1.1, 1.2, 1.3 的特征
        window == [0x16, 0x03, 0x00] || 
        window == [0x16, 0x03, 0x01] || 
        window == [0x16, 0x03, 0x02] || 
        window == [0x16, 0x03, 0x03] || 
        window == [0x16, 0x03, 0x04]
    });

    if is_possibly_tls {
        debug!("Detected possible TLS/SSL traffic");
        return Ok(ProxyProtocol::HTTPS);
    }

    debug!("Protocol detection inconclusive, treating as Unknown");
    Ok(ProxyProtocol::Unknown)
}




// Handle connection
async fn handle_connection(
    mut client_stream: TcpStream,
    target_addr: String,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let client_addr = client_stream.peer_addr()?.to_string();
    
    //debug!("New connection from {}", client_addr);
    
    // 检测协议
    let request_id = REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    
    // 检测协议
    let protocol = detect_protocol(&mut client_stream).await?;
    
    // 记录连接信息
    info!("[{}] New connection - Protocol: {:?}, Source: {}, Target: {}", 
          request_id, protocol, client_addr, target_addr);

    let target_stream = match TcpStream::connect(&target_addr).await {
        Ok(stream) => {
            debug!("Connected to target at {}", target_addr);
            stream
        },
        Err(e) => {
            debug!("Failed to connect to target: {}", e);
            return Err(Box::new(e));
        }
    };
    
    for socket in [&client_stream, &target_stream] {
        socket.set_nodelay(true)?;
    }

    let mut client_stream = client_stream;
    let mut target_stream = target_stream;

    match copy_bidirectional(
        &mut client_stream,
        &mut target_stream,
        client_addr.clone(),
        &target_addr,
    ).await {
        Ok(_) => debug!("Connection handled successfully for {}", client_addr),
        Err(e) => debug!("Error handling connection for {}: {}", client_addr, e),
    }

    Ok(())
}
// // 修改 handle_connection 函数支持 MITM
// pub async fn handle_connection(
//     mut client_stream: TcpStream,
//     target_addr: String,
//     ca_cert: Arc<Certificate>,
// ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//     let client_addr = client_stream.peer_addr()?.to_string();
//     let protocol = detect_protocol(&mut client_stream).await?;

//     match protocol {
//         ProxyProtocol::HTTPS => {
//             handle_https_mitm(client_stream, &target_addr, ca_cert, client_addr).await
//         }
//         ProxyProtocol::HTTP => {
//             let mut target_stream = TcpStream::connect(&target_addr).await?;
//             copy_bidirectional(
//                 &mut client_stream,
//                 &mut target_stream,
//                 client_addr,
//                 &target_addr,
//             ).await
//         }
//         ProxyProtocol::Unknown => {
//             Err("Unknown protocol".into())
//         }
//     }
// }

use once_cell::sync::OnceCell;
static LOGGER: OnceCell<()> = OnceCell::new();
use std::result::Result; // 显式引入 Result

// Set up a proxy
pub async fn start_proxy(config: ProxyConfig) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // set_logger()?;
    info!("Starting proxy server...");
    
    let listener = match TcpListener::bind(&config.listen_addr).await {
        Ok(l) => {
            info!("Proxy listening on {}", config.listen_addr);
            info!("Forwarding to {}", config.target_addr);
            l
        },
        Err(e) => {
            error!("Failed to bind to {}: {}", config.listen_addr, e);
            return Err(Box::new(e));
        }
    };

    let target_addr = config.target_addr.clone();

    loop {
        tokio::select! {
            accept_result = listener.accept() => {
                match accept_result {
                    Ok((client_stream, addr)) => {
                        info!("New connection from: {}", addr);
                        let target_addr = target_addr.clone();
                        tokio::spawn(async move {
                            if let Err(e) = handle_connection(client_stream, target_addr).await {
                                error!("Connection error for {}: {}", addr, e);
                            }
                        });
                    }
                    Err(e) => {
                        error!("Accept error: {}", e);
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                info!("Shutting down proxy server...");
                break;
            }
        }
    }
    Ok(())
}

// Start running the program
pub async fn run() {
    let args: Vec<String> = std::env::args().collect();
        
    // 查找 --listen 和 --target 参数
    let listen = args.iter()
        .position(|x| x == "--listen")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.to_string())
        .unwrap_or_else(|| "127.0.0.1:8081".to_string());
        
    let target = args.iter()
        .position(|x| x == "--target")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.to_string())
        .unwrap_or_else(|| "127.0.0.1:7890".to_string());

    info!("[+] Starting proxy with listen: {}, target: {}", listen, target);

    let config = ProxyConfig {
        listen_addr: listen,
        target_addr: target,
    };

    tokio::spawn(async move {
        if let Err(e) = start_proxy(config).await {
            eprintln!("Proxy error: {}", e);
        }
    });

    sleep(Duration::from_secs(1)).await;
    tokio::signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
}

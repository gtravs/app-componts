#![allow(unused)]
pub const DEFAULT_BUF_SIZE: usize = 8192;

use std::thread::JoinHandle;
use anyhow::Context;
use super::prelude::*;
use super::session::{Session};
use std::time::Duration;
use tokio::{io::{copy_bidirectional, AsyncReadExt, AsyncWriteExt}, net::TcpStream, sync::Mutex, time::sleep};
use tokio_rustls::{TlsAcceptor, TlsConnector};
use crate::ProxyLogEntry;
use crate::proxy_info;
use tokio::sync::broadcast;
// mod prelude;
// mod ca_cert;
// mod session;
// mod debug_stream;
// mod copy;


// set_proxy_port
async fn set_proxy_port(host: String, port: String) -> Result<tokio::net::TcpListener, anyhow::Error> {
    let addr = format!("{}:{}", host, port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("[+] Listening on {}\n", &*addr);
    Ok(listener)
}


pub async fn entry(shutdown_tx: broadcast::Sender<()>,proxy_host:String,proxy_port: String) -> Result<(), anyhow::Error> {
    // test code
    let listener = set_proxy_port(proxy_host, proxy_port).await.context("[-] Failed to set_proxy_port func error: bad listener.")?;
    let cacert = Arc::new(generate_ca_certificate().await.context("[-] Failed to generate ca certificate")?);
    let mut rx = shutdown_tx.subscribe();
    let mut active_connections = Vec::new();

    loop {
        tokio::select! {
            accept_result = listener.accept() => {
                match accept_result {
                    Ok((stream, addr)) => {
                        let uuid = uuid::Uuid::new_v4();
                        let session_id = u32::from_le_bytes(uuid.as_bytes()[0..4].try_into().unwrap());
                        let session = Arc::new(Mutex::new(Session::new(session_id, stream).unwrap()));
                        let ca_cert = Arc::clone(&cacert);
                        let session_clone = Arc::clone(&session);
                        
                        let handle = tokio::spawn(async move {
                            let mut session_lock = session_clone.lock().await;
                            // ... 原有的会话处理逻辑 ...
                            session_lock.session_connect(addr).await;
                            let method = session_lock.request.method.clone();
                            let url = session_lock.request.url.clone();
                            let initial_data = session_lock.initial_data.clone();
                            let header_host = session_lock.request.host.clone();
                            
                            println!("[Session {}] Request Method: {:?}, URL: {}", session_id, method, url);
                            
                            match method {
                                Method::CONNECT => {
                                    let url_split: Vec<&str> = url.split(":").collect();
                                    let host = url_split[0].to_string();
                                    let port = url_split[1].to_string();
                                    session_lock.handle_https(host, port, ca_cert).await;
                                }
                                _ => {
                                    let (host, port) = match header_host.split(':').collect::<Vec<&str>>().as_slice() {
                                        [host] => (host.to_string(), "80".to_string()),
                                        [host, port] => (host.to_string(), port.to_string()),
                                        _ => ("".to_string(), "80".to_string())
                                    };
                                    session_lock.handle_http(host, port, initial_data).await;
                                }
                            }
                            proxy_info!(
                                session_lock.session_id.to_string().as_str(),
                                session_lock.request.host.as_str(),
                                session_lock.request.method.as_str(),
                                session_lock.request.url.as_str(),
                                session_lock.response.status_code.as_str(),
                                session_lock.response.to_bytes().len().to_string().as_str()
                            );
                        });
                        
                        active_connections.push(handle);
                    }
                    Err(e) => {
                        eprintln!("[-] Failed to listener accept: {:?}", e);
                    }
                }
            }
            _ = rx.recv() => {
                println!("Shutdown signal received, closing all connections...");
                // 等待所有活动连接完成
                for handle in active_connections {
                    let _ = handle.await;
                }
                println!("All connections closed");
                return Ok(());
            }
        }

    }

    Ok(())
}

// #[cfg(test)]
// mod test {
//     use session::Session;

//     use super::*;   
//     // test模块测试
//     #[tokio::test]
//     async fn task_test_run() {
//         let _ = entry().await.context("[-] Failed to entry.");
//     }
// }

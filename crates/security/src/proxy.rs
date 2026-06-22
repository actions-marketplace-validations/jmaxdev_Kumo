use std::collections::HashSet;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

pub async fn start_proxy(allowed_domains: HashSet<String>) -> std::io::Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();

    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let allowed = allowed_domains.clone();
            tokio::spawn(async move {
                let mut buf = [0u8; 4096];
                if let Ok(n) = stream.read(&mut buf).await {
                    if n == 0 {
                        return;
                    }
                    let req = String::from_utf8_lossy(&buf[..n]);

                    if req.starts_with("CONNECT ") {
                        let parts: Vec<&str> = req.split_whitespace().collect();
                        if parts.len() >= 2 {
                            let host_port = parts[1];
                            let host = host_port.split(':').next().unwrap_or("");

                            if allowed.contains(host) {
                                if let Ok(mut server) = TcpStream::connect(host_port).await {
                                    let _ = stream
                                        .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
                                        .await;
                                    let (mut cr, mut cw) = stream.split();
                                    let (mut sr, mut sw) = server.split();

                                    let client_to_server = tokio::io::copy(&mut cr, &mut sw);
                                    let server_to_client = tokio::io::copy(&mut sr, &mut cw);

                                    let _ = tokio::try_join!(client_to_server, server_to_client);
                                } else {
                                    let _ =
                                        stream.write_all(b"HTTP/1.1 502 Bad Gateway\r\n\r\n").await;
                                }
                            } else {
                                eprintln!("\n🚨 \x1b[31mFirewall Blocked Outbound Connection to:\x1b[0m {}", host);
                                let _ = stream.write_all(b"HTTP/1.1 403 Forbidden\r\n\r\n").await;
                            }
                        }
                    } else {
                        let _ = stream.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n").await;
                    }
                }
            });
        }
    });

    Ok(port)
}

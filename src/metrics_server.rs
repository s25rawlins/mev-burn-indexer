use crate::error::AppError;
use crate::metrics;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{error, info};

/// Start the metrics HTTP server
pub async fn start_metrics_server(port: u16) -> Result<(), AppError> {
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await
        .map_err(|e| AppError::Config(format!("Failed to bind metrics server: {}", e)))?;
    
    info!("Metrics server listening on {}", addr);

    loop {
        match listener.accept().await {
            Ok((mut socket, _)) => {
                tokio::spawn(async move {
                    let mut buffer = [0; 1024];
                    
                    // Read the request
                    if let Err(e) = socket.read(&mut buffer).await {
                        error!("Failed to read from socket: {}", e);
                        return;
                    }

                    // Parse the request to check if it's for /metrics
                    let request = String::from_utf8_lossy(&buffer);
                    
                    if request.starts_with("GET /metrics") {
                        // Gather metrics
                        let metrics_output = metrics::gather_metrics();
                        
                        // Build HTTP response
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: text/plain; version=0.0.4\r\nContent-Length: {}\r\n\r\n{}",
                            metrics_output.len(),
                            metrics_output
                        );

                        // Send response
                        if let Err(e) = socket.write_all(response.as_bytes()).await {
                            error!("Failed to write to socket: {}", e);
                        }
                    } else if request.starts_with("GET /health") {
                        // Health check endpoint
                        let response = "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 2\r\n\r\nOK";
                        
                        if let Err(e) = socket.write_all(response.as_bytes()).await {
                            error!("Failed to write to socket: {}", e);
                        }
                    } else {
                        // 404 for other paths
                        let response = "HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\n\r\nNot Found";
                        
                        if let Err(e) = socket.write_all(response.as_bytes()).await {
                            error!("Failed to write to socket: {}", e);
                        }
                    }
                });
            }
            Err(e) => {
                error!("Failed to accept connection: {}", e);
            }
        }
    }
}

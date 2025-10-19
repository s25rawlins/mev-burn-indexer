use crate::error::AppError;
use crate::metrics;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{error, info};

/// Start the metrics HTTP server with automatic port fallback.
/// 
/// Attempts to bind to the requested port first. If that port is already in use,
/// automatically tries alternate ports (up to 10 attempts) to ensure the metrics
/// server can start even if the default port is occupied by another process.
pub async fn start_metrics_server(port: u16) -> Result<(), AppError> {
    const MAX_PORT_ATTEMPTS: u16 = 10;
    
    let mut last_error = None;
    
    // Try the requested port and up to MAX_PORT_ATTEMPTS alternatives
    for attempt in 0..MAX_PORT_ATTEMPTS {
        let try_port = port + attempt;
        let addr = SocketAddr::from(([0, 0, 0, 0], try_port));
        
        match TcpListener::bind(addr).await {
            Ok(listener) => {
                if attempt > 0 {
                    info!(
                        requested_port = port,
                        actual_port = try_port,
                        "Requested port was in use, bound to alternate port"
                    );
                } else {
                    info!(port = try_port, "Metrics server listening");
                }
                
                // Successfully bound, start serving
                return serve_metrics(listener).await;
            }
            Err(e) => {
                last_error = Some((try_port, e));
            }
        }
    }
    
    // All port attempts failed
    let (failed_port, error) = last_error.unwrap();
    Err(AppError::Config(format!(
        "Failed to bind metrics server after {} attempts (ports {}-{}): {}. \
         All ports are in use. Try: \
         1) Stop processes using these ports (find with: lsof -i :{}-{} or ss -tulpn | grep :{}), \
         2) Set METRICS_PORT to a different range",
        MAX_PORT_ATTEMPTS, port, failed_port, error, port, failed_port, port
    )))
}

/// Serve metrics on the bound listener.
async fn serve_metrics(listener: TcpListener) -> Result<(), AppError> {
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

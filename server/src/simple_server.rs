//! Simple HTTP server for testing client authentication

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

fn main() {
    println!("Starting simple HTTP server on port 3000...");
    let listener = TcpListener::bind("127.0.0.1:3000").unwrap();

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                thread::spawn(|| {
                    handle_connection(stream);
                });
            }
            Err(e) => {
                eprintln!("Connection failed: {}", e);
            }
        }
    }
}

fn handle_connection(mut stream: TcpStream) {
    let mut buffer = [0; 2048];
    
    // Read the HTTP request
    match stream.read(&mut buffer) {
        Ok(size) => {
            let request_str = String::from_utf8_lossy(&buffer[0..size]).to_string();
            let request_line = request_str.lines().next().unwrap_or_default();
            println!("Request: {}", request_line);
            
            if request_line.starts_with("GET /health") {
                // Health check endpoint
                let response = "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\r\nOK";
                stream.write_all(response.as_bytes()).unwrap();
            } else if request_line.starts_with("POST /license/verify") {
                // Extract key from body (simplified approach)
                let license_key = if request_str.contains("TEST-DEV-LICENSE-KEY") {
                    "TEST-DEV-LICENSE-KEY"
                } else if request_str.contains("TEST-PRO-LICENSE-KEY") {
                    "TEST-PRO-LICENSE-KEY"
                } else if request_str.contains("TEST-EXPIRED-LICENSE") {
                    "TEST-EXPIRED-LICENSE"
                } else if request_str.contains("DEV-") {
                    "DEV-CUSTOM"
                } else {
                    "INVALID"
                };
                
                // Prepare response based on key
                let json_response = match license_key {
                    "TEST-DEV-LICENSE-KEY" => {
                        r#"{"valid":true,"user_email":"developer@example.com","subscription_type":"developer","expires_at":1735686000,"features":["basic","advanced","developer"],"message":null}"#
                    },
                    "TEST-PRO-LICENSE-KEY" => {
                        r#"{"valid":true,"user_email":"pro@example.com","subscription_type":"professional","expires_at":1735686000,"features":["basic","advanced","professional"],"message":null}"#
                    },
                    "TEST-EXPIRED-LICENSE" => {
                        r#"{"valid":false,"user_email":"expired@example.com","subscription_type":"basic","expires_at":1609459200,"features":["basic"],"message":"License has expired"}"#
                    },
                    "DEV-CUSTOM" => {
                        r#"{"valid":true,"user_email":"developer@localhost","subscription_type":"development","expires_at":1735686000,"features":["all"],"message":null}"#
                    },
                    _ => {
                        r#"{"valid":false,"user_email":null,"subscription_type":null,"expires_at":null,"features":[],"message":"Invalid license key"}"#
                    }
                };
                
                println!("License verification result for key: {} = {}", license_key, json_response.contains("\"valid\":true"));
                
                // Send HTTP response
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                    json_response.len(),
                    json_response
                );
                stream.write_all(response.as_bytes()).unwrap();
            } else {
                // 404 Not Found for other endpoints
                let response = "HTTP/1.1 404 NOT FOUND\r\n\r\n";
                stream.write_all(response.as_bytes()).unwrap();
            }
        }
        Err(e) => {
            eprintln!("Error reading from connection: {}", e);
        }
    }
}
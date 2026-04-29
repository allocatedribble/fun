use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
};

use game_shared::{BACKEND_SERVER_ADDR, GAME_TITLE};

fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind(BACKEND_SERVER_ADDR)?;
    println!("Starting {GAME_TITLE} backend server scaffold on {BACKEND_SERVER_ADDR}.");

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => handle_connection(&mut stream)?,
            Err(error) => eprintln!("Backend accept error: {error}"),
        }
    }

    Ok(())
}

fn handle_connection(stream: &mut TcpStream) -> std::io::Result<()> {
    let mut request_buffer = [0; 1024];
    let _ = stream.read(&mut request_buffer)?;

    let body = r#"{"status":"ok","service":"fun-backend"}"#;
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    )
}

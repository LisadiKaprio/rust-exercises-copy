use std::net::SocketAddr;

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{tcp::WriteHalf, TcpListener, TcpStream, ToSocketAddrs},
    sync::broadcast::{self, error::RecvError, Receiver, Sender},
};

#[tokio::main]
pub(crate) async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    accept_loop("127.0.0.1:8080").await
}

async fn accept_loop(
    addr: impl ToSocketAddrs,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let listener = TcpListener::bind(addr).await?;

    let (tx, _rx) = broadcast::channel(10);

    println!("Connected to {}", listener.local_addr()?.to_string());

    loop {
        handle_client(&listener, tx.clone()).await;
    }
}

async fn handle_client(
    listener: &TcpListener,
    tx: Sender<(String, SocketAddr)>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut rx = tx.subscribe();
    let (mut stream, addr) = listener.accept().await.unwrap();
    println!("Client joined...");

    tokio::spawn(async move {
        handle_communication(&mut stream, tx.clone(), &mut rx, &addr).await;
    });
    Ok(())
}

async fn handle_communication(
    stream: &mut TcpStream,
    tx: Sender<(String, SocketAddr)>,
    rx: &mut Receiver<(String, SocketAddr)>,
    addr: &SocketAddr,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (read_half, mut write_half) = stream.split();
    let mut reader = BufReader::new(read_half);

    write_half
        .write_all(&"Input your name: ".as_bytes())
        .await
        .unwrap();
    let mut name = String::new();
    reader.read_line(&mut name).await?;
    // let name = match first_line {
    //     None => Err("peer disconnected immediately")?,
    //     Some(line) => line,
    // };
    println!("name = {}", name);

    let mut line = String::new();

    loop {
        tokio::select! {
            _result = reader.read_line(&mut line) => {
                tx.send((line.clone(), addr.clone())).unwrap();
                line.clear();
            }
            result = rx.recv() => {
                receive_message(result, &mut write_half, &mut line, addr).await?
            }
        }
    }
    Ok(())
}

async fn receive_message(
    result: Result<(String, SocketAddr), RecvError>,
    write_half: &mut WriteHalf<'_>,
    line: &mut String,
    addr: &SocketAddr,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (msg, other_addr) = result.unwrap();

    if addr != &other_addr {
        write_half.write_all(&msg.as_bytes()).await.unwrap();
    }
    line.clear();
    Ok(())
}

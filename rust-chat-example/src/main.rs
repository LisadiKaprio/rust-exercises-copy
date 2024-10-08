use std::net::SocketAddr;

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{
        tcp::{ReadHalf, WriteHalf},
        TcpListener,
    },
    sync::broadcast::{self, error::RecvError, Receiver, Sender},
};

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("localhost:8080").await.unwrap();

    let (tx, _rx) = broadcast::channel(10);
    println!("Connected to localhost:8080");

    loop {
        handle_client(&listener, tx.clone()).await;
    }
}

async fn handle_client(listener: &TcpListener, tx: Sender<(String, SocketAddr)>) {
    let mut rx = tx.subscribe();
    let (mut socket, addr) = listener.accept().await.unwrap();
    println!("Client joined...");

    tokio::spawn(async move {
        let (read_half, mut write_half) = socket.split();
        let mut reader = BufReader::new(read_half);

        let mut line = String::new();
        write_half.write_all(&"Welcome!".as_bytes()).await.unwrap();

        loop {
            handle_communication(
                &mut write_half,
                &mut reader,
                &mut line,
                tx.clone(),
                &mut rx,
                &addr,
            )
            .await;
        }
    });
}

async fn handle_communication(
    write_half: &mut WriteHalf<'_>,
    reader: &mut BufReader<ReadHalf<'_>>,
    line: &mut String,
    tx: Sender<(String, SocketAddr)>,
    rx: &mut Receiver<(String, SocketAddr)>,
    addr: &SocketAddr,
) {
    tokio::select! {
        _result = reader.read_line(line) => {
            tx.send((line.clone(), addr.clone())).unwrap();
            line.clear();
        }
        result = rx.recv() => {
            receive_message(result, write_half, line, addr).await
        }
    }
}

async fn receive_message(
    result: Result<(String, SocketAddr), RecvError>,
    write_half: &mut WriteHalf<'_>,
    line: &mut String,
    addr: &SocketAddr,
) {
    let (msg, other_addr) = result.unwrap();

    if addr != &other_addr {
        write_half.write_all(&msg.as_bytes()).await.unwrap();
    }
    line.clear();
}

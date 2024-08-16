use std::sync::mpsc::RecvError;

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{
        tcp::{ReadHalf, WriteHalf},
        TcpListener,
    },
    sync::broadcast::{self, Receiver, Sender},
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

async fn handle_client(listener: &TcpListener, tx: Sender<String>) {
    let mut rx = tx.subscribe();
    let (mut socket, _addr) = listener.accept().await.unwrap();
    println!("Client joined...");

    tokio::spawn(async move {
        let (read_half, mut write_half) = socket.split();
        let mut reader = BufReader::new(read_half);

        let mut line = String::new();

        loop {
            handle_communication(&mut write_half, &mut reader, &mut line, tx.clone(), &mut rx)
                .await;
        }
    });
}

async fn handle_communication(
    write_half: &mut WriteHalf<'_>,
    reader: &mut BufReader<ReadHalf<'_>>,
    line: &mut String,
    tx: Sender<String>,
    rx: &mut Receiver<String>,
) {
    println!("cycle start!");

    write_half
        .write_all(&"Write your message here: ".as_bytes())
        .await
        .unwrap();

    tokio::select! {
        _result = reader.read_line(line) => {
            tx.send(line.clone()).unwrap();
            line.clear();
        }
        result = rx.recv() => {
            receive_message(result.unwrap(), write_half, line).await
        }
    }
    println!("cycle end!");
}

async fn receive_message(msg: String, write_half: &mut WriteHalf<'_>, line: &mut String) {
    let msg = format!("\r\n\r\n{}", msg);

    write_half.write_all(&msg.as_bytes()).await.unwrap();
    line.clear();
}

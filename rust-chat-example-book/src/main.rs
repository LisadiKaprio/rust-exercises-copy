use std::{
    collections::{hash_map::Entry, HashMap},
    future::Future,
    net::SocketAddr,
};

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{tcp::OwnedWriteHalf, TcpListener, TcpStream, ToSocketAddrs},
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    task::JoinHandle,
};

type BoxedResult<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

fn spawn_and_log_error<F>(function: F) -> JoinHandle<()>
where
    F: Future<Output = BoxedResult<()>> + Send + 'static,
{
    tokio::spawn(async move {
        if let Err(e) = function.await {
            eprintln!("{}", e)
        }
    })
}

// NOTE:    the code from the book implemented here
//          assumes that you write messages formatted like this:
//          "other_user_1, other_user_2: Hello world!"

#[tokio::main]
pub(crate) async fn main() -> BoxedResult<()> {
    accept_loop("127.0.0.1:8080").await
}

async fn accept_loop(addr: impl ToSocketAddrs) -> BoxedResult<()> {
    let listener = TcpListener::bind(addr).await?;
    println!("Connected to {}", listener.local_addr()?.to_string());

    let (broker_sender, broker_receiver) = unbounded_channel();
    let _broker = tokio::spawn(broker_loop(broker_receiver));

    while let Ok((stream, addr)) = listener.accept().await {
        println!("Client joined...");
        spawn_and_log_error(connection_loop(broker_sender.clone(), stream));
    }
    Ok(())
}
enum Event {
    NewClient {
        name: String,
        write_half: OwnedWriteHalf,
    },
    Message {
        from: String,
        to: Vec<String>,
        msg: String,
    },
}

async fn broker_loop(mut events: UnboundedReceiver<Event>) {
    let mut clients: HashMap<String, UnboundedSender<String>> = HashMap::new();

    loop {
        let event = match events.recv().await {
            Some(event) => event,
            None => break,
        };

        match event {
            Event::NewClient {
                name,
                mut write_half,
            } => match clients.entry(name.clone()) {
                Entry::Occupied(_) => (),
                Entry::Vacant(entry) => {
                    let (client_sender, mut client_receiver) = unbounded_channel();
                    entry.insert(client_sender);
                    spawn_and_log_error(async move {
                        receive_messages_on_loop(&mut client_receiver, &mut write_half).await
                    });
                }
            },
            Event::Message { from, to, msg } => {
                for addr in to {
                    if let Some(client) = clients.get(&addr) {
                        let message = format!("{:?}: {:?}\n", from, &msg);
                        let sending_attempt = client.send(message.clone());
                        if let Err(e) = sending_attempt {
                            eprintln!(
                                "Error while receiving message from {from} on {:?}: {e}",
                                &addr
                            );
                        }
                    }
                }
            }
        }
    }
}

async fn receive_messages_on_loop(
    client_receiver: &mut UnboundedReceiver<String>,
    write_half: &mut OwnedWriteHalf,
) -> BoxedResult<()> {
    loop {
        let message = client_receiver.recv().await;
        match message {
            Some(message) => write_half.write_all(&message.as_bytes()).await?,
            None => break,
        }
    }
    Ok(())
}

async fn connection_loop(
    broker_sender: UnboundedSender<Event>,
    stream: TcpStream,
) -> BoxedResult<()> {
    tokio::spawn(async move {
        let communication = handle_communication(broker_sender.clone(), stream).await;
        if let Err(e) = communication {
            eprintln!("Error on handling communication: {e}");
        }
    });
    Ok(())
}

async fn handle_communication(
    broker_sender: UnboundedSender<Event>,
    stream: TcpStream,
) -> BoxedResult<()> {
    let (read_half, mut write_half) = stream.into_split();
    let reader = BufReader::new(read_half);
    let mut lines = reader.lines();

    write_half
        .write_all(&"Input your name: ".as_bytes())
        .await
        .unwrap();
    let name = match lines.next_line().await? {
        None => Err("peer disconnected immediately")?,
        Some(line) => line,
    };
    println!("{} joined.", name);

    // deliberate unwrap() for broker actions, as mentioned in book
    broker_sender
        .send(Event::NewClient {
            name: name.clone(),
            write_half,
        })
        .unwrap();

    loop {
        if let Some(line) = lines.next_line().await? {
            println!("{:?}", &line);
            let (dest, message) = match line.find(':') {
                None => continue,
                Some(idx) => (&line[..idx], line[idx + 1..].trim()),
            };
            let dest: Vec<String> = dest
                .split(',')
                .map(|name| name.trim().to_string())
                .collect();
            let message = message.trim().to_string();

            // deliberate unwrap() for broker actions, as mentioned in book
            broker_sender
                .send(Event::Message {
                    from: name.clone(),
                    to: dest,
                    msg: message,
                })
                .unwrap();
        } else {
            break;
        }
    }
    Ok(())
}

// async fn receive_message(
//     result: Result<(String, SocketAddr), RecvError>,
//     write_half: &mut WriteHalf<'_>,
//     addr: &SocketAddr,
// ) -> BoxedResult<()> {
//     let (msg, other_addr) = result.unwrap();

//     if addr != &other_addr {
//         write_half.write_all(&msg.as_bytes()).await?;
//     }
//     Ok(())
// }

use std::{
    collections::{hash_map::Entry, HashMap},
    future::Future,
    sync::Arc,
};

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{tcp::OwnedWriteHalf, TcpListener, TcpStream, ToSocketAddrs},
    signal::ctrl_c,
    sync::{
        mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
        Notify,
    },
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

// start by cargo run
// then connect from different terminal instances using `telnet localhost 8080`

// NOTE:    the code from the book implemented here
//          assumes that you write messages formatted like this:
//          "other_user_1, other_user_2: Hello world!"

#[tokio::main]
pub(crate) async fn main() -> BoxedResult<()> {
    accept_loop("127.0.0.1:8080").await
}

async fn accept_loop(addr: impl ToSocketAddrs) -> BoxedResult<()> {
    // starting server at localhost port 8080
    let listener = TcpListener::bind(addr).await?;
    println!("Connected to {}", listener.local_addr()?.to_string());

    // creating a channel that can send and receive signals
    let (broker_sender, broker_receiver) = unbounded_channel();

    // creating an async loop that receives messages
    let broker = tokio::spawn(broker_loop(broker_receiver));

    // declaring empty notification that can be cloned asynchronously to many processes when needed
    let shutdown_notifaction = Arc::new(Notify::new());

    loop {
        // each iteration, we watch out for either clients joining on server listener,
        // or for shutdown initiated by user getting detected
        tokio::select! {
            Ok((stream, _addr)) = listener.accept() => {
                println!("Client joined...");

                // start loop that will handle this client's communication (by creating another send-receive channel)
                spawn_and_log_error(handle_client_communication(broker_sender.clone(), stream, shutdown_notifaction.clone()));
            },
            _ = ctrl_c() => break,
        }
    }
    // we only arrive here if user shut down the server
    println!("Shutting down server...");

    // we use the notification to notify all awaiting still running processes about the shutdown?
    shutdown_notifaction.notify_waiters();

    // we get rid of the broker part that sends signals to clients
    // that way, the broker loop will end once all tasks are done, since no more messages can be sent?
    drop(broker_sender);

    // we wait for broker to finish what it was doing
    broker.await?;
    Ok(())
}
enum Event {
    NewClient {
        name: String,
        write_half: OwnedWriteHalf,
    },
    Message {
        from_name: String,
        to_names: Vec<String>,
        message: String,
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
            Event::Message {
                from_name: from,
                to_names: to,
                message: msg,
            } => {
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
    for client in &clients {
        let sending_attempt = client
            .1
            .send("Admin is shutting down the server...".to_string());
        if let Err(e) = sending_attempt {
            eprintln!(
                "Error while sending shutdown message on {:?}: {e}",
                &client.0
            );
        }
    }
    drop(clients);
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

async fn handle_client_communication(
    broker_sender: UnboundedSender<Event>,
    stream: TcpStream,
    shutdown_notification: Arc<Notify>,
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
        tokio::select! {
            Ok(Some(line)) = lines.next_line() => {
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
                        from_name: name.clone(),
                        to_names: dest,
                        message,
                    })
                    .unwrap();
            },
            _ = shutdown_notification.notified() => break,
        }
    }
    Ok(())
}

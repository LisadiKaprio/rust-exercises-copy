use std::{
    collections::{hash_map::Entry, HashMap},
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
};

use crate::utils::{spawn_and_log_error, BoxedResult};

async fn accept_loop(addr: impl ToSocketAddrs) -> BoxedResult<()> {
    let listener = TcpListener::bind(addr).await?;
    println!("Connected to {}", listener.local_addr()?.to_string());

    let (broker_sender, broker_receiver) = unbounded_channel();
    let broker = tokio::spawn(broker_loop(broker_receiver));

    let shutdown_notifaction = Arc::new(Notify::new());

    loop {
        tokio::select! {
            Ok((stream, _addr)) = listener.accept() => {
                println!("Client joined...");

                spawn_and_log_error(handle_client_communication(broker_sender.clone(), stream, shutdown_notifaction.clone()));
            },
            _ = ctrl_c() => break,
        }
    }

    println!("Shutting down server...");
    shutdown_notifaction.notify_waiters();
    drop(broker_sender);
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
                let all_command = "all".to_string();

                if to.contains(&all_command) {
                    for client in &clients {
                        if client.0 != &from {
                            let _ = send_message(&from, &all_command, &msg, client.1).await;
                        }
                    }
                } else {
                    for addr in to {
                        if let Some(client) = clients.get(&addr) {
                            let _ = send_message(&from, &addr, &msg, client).await;
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

async fn send_message(from: &str, to: &str, msg: &str, client: &UnboundedSender<String>) {
    let message = format!("{:?}: {:?}\n", from, &msg);
    let sending_attempt = client.send(message.clone());
    if let Err(e) = sending_attempt {
        eprintln!("Error while receiving message from {from} on {:?}: {e}", to);
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

    // lines.next_line() should be cleaned up, to avoid f.e. backspace appearing like "arai\u{8}\u{8}iana"
    // also, \u{1b}[Auser2
    let name = match lines.next_line().await? {
        None => "_".to_string(),
        Some(line) => line,
    };
    println!("{} joined.", name);

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

use async_trait::async_trait;
use russh::keys::*;
use russh::server::{Msg, Server as _, Session};
use russh::*;
use server::Config;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::{TcpListener, ToSocketAddrs};
use tokio::sync::Mutex;

use crate::utils::BoxedResult;

pub async fn start_russh_server(addr: impl ToSocketAddrs) -> BoxedResult<()> {
    let mut sh = Server {
        clients: Arc::new(Mutex::new(HashMap::new())),
        id: 0,
    };
    let listener = TcpListener::bind(addr).await?;
    sh.connect(listener).await?;
    Ok(())
}

#[derive(Clone)]
struct Server {
    clients: Arc<Mutex<HashMap<(usize, ChannelId), russh::server::Handle>>>,
    id: usize,
}

impl Server {
    async fn connect(&mut self, listener: TcpListener) -> Result<(), anyhow::Error> {
        let config = Config {
            inactivity_timeout: Some(std::time::Duration::from_secs(3600)),
            auth_rejection_time: std::time::Duration::from_secs(3),
            auth_rejection_time_initial: Some(std::time::Duration::from_secs(0)),
            keys: vec![russh_keys::key::KeyPair::generate_ed25519().unwrap()],
            ..Default::default()
        };

        self.run_on_socket(Arc::new(config), &listener).await?;
        println!("Connect function end");
        Ok(())
    }

    async fn post(&mut self, receiver_id: usize, data: CryptoVec) {
        let clients = self.clients.lock().await;
        let receiver_client = clients.iter().find(|((id, _), _)| id == &receiver_id);
        if let Some(((_, channel), s)) = receiver_client {
            let _ = s.data(*channel, data).await;
        }
    }
}

impl server::Server for Server {
    type Handler = Self;
    fn new_client(&mut self, _: Option<std::net::SocketAddr>) -> Self {
        let s = self.clone();
        self.id += 1;
        println!("Client joined. New client receives the id {}", { s.id });
        s
    }
}

#[async_trait]
impl server::Handler for Server {
    type Error = anyhow::Error;

    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        session: &mut Session,
    ) -> Result<bool, Self::Error> {
        let mut clients = self.clients.lock().await;
        let channel_id = channel.id().to_owned();

        clients.insert((self.id, channel_id), session.handle());

        let _ = session
            .handle()
            .data(
                channel_id,
                CryptoVec::from("The connection to the server was established!\r\n".to_string()),
            )
            .await;

        Ok(true)
    }

    async fn auth_publickey(
        &mut self,
        _: &str,
        _: &key::PublicKey,
    ) -> Result<server::Auth, Self::Error> {
        println!("New client authorized!");
        Ok(server::Auth::Accept)
    }

    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        // TODO: on receiver client, display which sender client the message came from

        // TODO: create separate functions for what happens in the match statement
        // TODO: create function that takes in receiver_channel_id and message_string and sends the message

        // TODO: create enum for the commands
        // TODO: create enum for messages that the client may receive on unexpected input
        //              f.e. empty input, no command, no receiver id argument, no message

        // TODO: clean disconnect on ctrl + c in server terminal
        //          -> clients should receive a notification about it
        //          -> potentially, clients should also be shut down
        // TODO: clean disconnect on ctrl + c in client terminals
        //          -> server should receive feedback about it and delete this client from its memory

        // TODO: ensure the server app can be run on any server + port
        //          -> (env file? command arguments when starting up?)
        // TODO: ensure the client app can customize server + port and ssh key location?

        let string = String::from_utf8_lossy(data);
        let string = string.trim();

        let split_input = string.split(' ');
        let input_words: Vec<&str> = split_input.collect();

        if input_words.len() == 0 {
            println!("Empty input from client id {}", &self.id);
            return Ok(());
        }

        match input_words[0] {
            "/message" => {
                if input_words.len() < 2 {
                    let _ = session
                        .handle()
                        .data(
                            channel,
                            CryptoVec::from(
                                "Input must include the receiver id, then message".to_string(),
                            ),
                        )
                        .await;
                    return Ok(());
                }
                let first_argument = &input_words[1];
                if let Ok(receiver_id) = first_argument.parse::<usize>() {
                    let message = input_words[2..].join(" ");
                    let _ = self.post(receiver_id, CryptoVec::from(message)).await;
                }
            }
            "/clients" => {
                let clients = self.clients.lock().await;
                let channel_ids: Vec<String> =
                    clients.keys().map(|(id, _)| id.to_string()).collect();
                let data = format!(
                    "\r\r\nFollowing client ids are available to be connected to: {}\r\n",
                    channel_ids.join(", ")
                );
                let data = format!("{data}Your client id is {}\r\r\n", self.id);

                let _ = session.handle().data(channel, CryptoVec::from(data)).await;
            }
            "/quit" => {
                // messages sent to client here cannot be received on client :/

                self.clients.lock().await.remove(&(self.id, channel));
                session.close(channel);
            }
            _ => {}
        }

        Ok(())
    }
}

use async_trait::async_trait;
use russh::keys::*;
use russh::server::{Msg, Server as _, Session};
use russh::*;
use server::Config;
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;
use tokio::net::{TcpListener, ToSocketAddrs};
use tokio::sync::Mutex;

use crate::utils::BoxedResult;

pub async fn start_russh_server(addr: impl ToSocketAddrs) -> BoxedResult<()> {
    let mut sh = Server {
        clients: Arc::new(Mutex::new(HashMap::new())),
        id: 0,
        client_connections: Arc::new(Mutex::new(HashMap::new())),
    };
    let listener = TcpListener::bind(addr).await?;
    sh.connect(listener).await?;
    Ok(())
}

pub fn check_public_key<P: AsRef<Path>>(
    path: P,
    client_public_key: &key::PublicKey,
) -> Result<bool, russh_keys::Error> {
    let mut data = String::new();
    let mut file = File::open(path.as_ref())?;
    file.read_to_string(&mut data)?;

    let mut split = data.split("\n");

    while let Some(key_line) = split.next() {
        let mut key_line = key_line.split_whitespace();
        match (key_line.next(), key_line.next()) {
            (Some(_), Some(key)) => {
                let key = parse_public_key_base64(key)?;
                if &key == client_public_key {
                    return Ok(true);
                }
            }
            (Some(key), None) => {
                let key = parse_public_key_base64(key)?;
                if &key == client_public_key {
                    return Ok(true);
                }
            }
            _ => {}
        }
    }
    Ok(false)
}

#[derive(Clone)]
struct Server {
    clients: Arc<Mutex<HashMap<(usize, ChannelId), russh::server::Handle>>>,
    id: usize,
    client_connections: Arc<Mutex<HashMap<usize, usize>>>,
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

    async fn auth_publickey_offered(
        &mut self,
        _: &str,
        client_public_key: &key::PublicKey,
    ) -> Result<server::Auth, Self::Error> {
        let client_public_ssh_key_location =
            env::var("CLIENT_PUBLIC_SSH_KEY").expect("SERVER_PORT must be a valid number.");

        // let key_pair = load_public_key(client_public_ssh_key_location)?;

        if check_public_key(client_public_ssh_key_location, client_public_key)? {
            Ok(server::Auth::Accept)
        } else {
            eprintln!("Could not authenticate this client's key.");
            Err(russh::Error::NotAuthenticated.into())
        }
    }

    async fn auth_publickey(
        &mut self,
        _: &str,
        _: &key::PublicKey,
    ) -> Result<server::Auth, Self::Error> {
        // println!(
        //     "New client authorized! {}",
        //     String::from_utf16_lossy(&key.public_key_bytes())
        // );
        Ok(server::Auth::Accept)
    }

    // N9��;b▬‼��↓☻L�]9�茾�M�aox+dң��ʘ�↔-ǜ?��Q��☺�r�3�§3�c_����Vm♀�s§u�#��꙱♂���M�Weh��� ���u0}�☺2����O;[C�↕=xU♫���+�B��OIn"]O.◄�vW�d�↔¶���hO�\��$�2Р�)5tS�+��s↨�M[☺;H��▬♫n▼�S�→O��▲��T�↨*�>8d��,9�A&|��\�^��▼䶎D*�X↓ɜ[�����‼`{~-����xQ��TkGC���♣�o��♦�e�8S���►򿭑�% ‼&☻N1u♀Y↓7+�Z�N��y7��4�U�h�-�"8{h8�ӌ����Q��[�

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

        let string = String::from_utf8_lossy(data);
        let string = string.trim();

        let split_input = string.split(' ');
        let input_words: Vec<&str> = split_input.collect();

        if input_words.len() == 0 {
            println!("Empty input from client id {}", &self.id);
            return Ok(());
        }

        // TODO:    get saved client connections
        // let mut client_connections = self.client_connections.lock().await;

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
            _ => {
                // TODO: if no command was given, send message to last communicated to client

                // let current_client_connection =
                //     client_connections.iter().find(|(id, _)| id == &self.id);
                // if let Some((_, receiver_id)) = receiver_client {
                //     let message = input_words[0..].join(" ");
                //     let _ = self.post(receiver_id, CryptoVec::from(message)).await;
                // }
            }
        }

        Ok(())
    }
}

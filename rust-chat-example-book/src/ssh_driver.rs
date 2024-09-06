use anyhow::anyhow;
use async_trait::async_trait;
use dotenv::dotenv;
use russh::{
    client::{self, Session},
    ChannelId,
};
use russh_keys::{key, load_secret_key};
use std::{env, fs::File, io::Read, sync::Arc};
use tokio::io::{stdin, AsyncBufReadExt, AsyncWriteExt, BufReader};

mod utils;
use utils::BoxedResult;

#[tokio::main]
pub(crate) async fn main() -> BoxedResult<()> {
    // dotenv().ok();
    // let client_user = env::var("CLIENT_USER").expect("CLIENT_USER must be named in env file.");
    // let client_private_ssh_key_location = env::var("CLIENT_PRIVATE_SSH_KEY").expect("CLIENT_PRIVATE_SSH_KEY must be named in env file.");

    // let host =
    //     env::var("SERVER_HOST").expect("SERVER_HOST must be named in env file (f.e. 0.0.0.0).");
    // let port = env::var("SERVER_PORT").expect("SERVER_PORT must be named in env file (f.e. 2222).");
    // let port = port
    //     .parse::<u16>()
    //     .expect("SERVER_PORT must be a valid number.");

    let mut data = String::new();
    let mut file = File::open(r".\config")?;
    file.read_to_string(&mut data)?;

    let mut split = data.split("\r\n");

    match (split.next(), split.next(), split.next(), split.next()) {
        (Some(host), Some(port), Some(client_user), Some(client_private_ssh_key_location)) => {
            let port = port
                .parse::<u16>()
                .expect("SERVER_PORT must be a valid number.");
            start_ssh_driver(client_user, client_private_ssh_key_location, host, port).await
        }
        _ => Err(anyhow!("No config")),
    }
}

// Read data from server ->     go to Client implementation -> data()
// Send data to server ->       go to tokio::spawn -> reader.read_line match -> stream.write_all

// 1. Running client, client gets saved with its unique id
// 2. User types command "/connect 3"
// 3. Server interprets the message as command and saves a connection to client with id 3
//      ->  only if client with id 3 isn't already connected,
//          otherwise user receives "Cannot connect to 3 at the moment" from server
// 4. User types message
// 5. Server receives message, checks that user's client is already connected to client 3
// 6. Client 3 receives the message.

// type command "/clients" and see a list of available client ids

pub async fn start_ssh_driver(
    user: &str,
    private_key: &str,
    host: &str,
    port: u16,
) -> Result<(), anyhow::Error> {
    let key_pair = load_secret_key(private_key, None)?;

    let config = client::Config { ..<_>::default() };
    let config = Arc::new(config);
    let sh = Client {};
    let mut session = client::connect(config, (host, port), sh).await?;

    let _auth_res = session
        .authenticate_publickey(user, Arc::new(key_pair))
        .await?;
    // println!("auth_res: {}", auth_res);

    let channel = session.channel_open_session().await?;

    // let _ = channel
    //     .data("Hello from client!".to_string().as_bytes())
    //     .await;

    let mut stream = channel.into_stream();
    let stdin = stdin();
    let mut reader = BufReader::new(stdin);

    let mut line_in = String::new();

    match tokio::spawn(async move {
        loop {
            match reader.read_line(&mut line_in).await {
                Err(_) => break,
                _ => {
                    stream.write_all(&line_in.as_bytes()).await?;
                    line_in = String::new();
                }
            }
        }
        anyhow::Ok::<()>(())
    })
    .await?
    {
        _ => {}
    }
    return Ok(());
}

struct Client {}

#[async_trait]
impl client::Handler for Client {
    type Error = anyhow::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &key::PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }

    async fn data(
        &mut self,
        _channel: ChannelId,
        data: &[u8],
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        let string = String::from_utf8_lossy(data);
        let string = string.trim().to_string();
        println!("{string}");
        Ok(())
    }
}

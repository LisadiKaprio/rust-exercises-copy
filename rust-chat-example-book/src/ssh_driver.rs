use async_trait::async_trait;
use russh::{client, CryptoVec};
use russh_keys::{key, load_secret_key};
use std::{net::SocketAddr, str::FromStr, sync::Arc};
use tokio::io::{stdin, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

mod utils;
use utils::BoxedResult;

#[tokio::main]
pub(crate) async fn main() -> BoxedResult<()> {
    start_ssh_driver(
        "WegesrandUser".to_string(),
        r"C:\Users\WegesrandUser\.ssh\id_rsa".to_string(),
        "run".to_string(),
    )
    .await
}

// Starts the ssh bridge driver
pub async fn start_ssh_driver(
    user: String,
    private_key_path: String,
    run_command: String,
) -> Result<(), anyhow::Error> {
    // Open SSH Session
    let key_pair = load_secret_key(private_key_path, None)?;

    let config = client::Config { ..<_>::default() };
    let config = Arc::new(config);
    let sh = Client {};
    let mut session = client::connect(config, ("localhost", 2222), sh).await?;

    let _auth_res = session
        .authenticate_publickey(user, Arc::new(key_pair))
        .await?;

    // Create new channel
    let channel = session.channel_open_session().await.unwrap();
    println!("channel id on client is {}", channel.id());

    let _ = channel
        .signal(russh::Sig::Custom("Hello from client!".to_string()))
        .await;

    let mut stream = channel.into_stream();

    // First Command
    let mut first_com = run_command;
    first_com.push_str("\n");
    stream.write_all(&first_com.as_bytes()).await.unwrap();

    // Start async stuff
    let stdin = stdin();
    let mut reader = BufReader::new(stdin);

    let mut line_in = String::new();
    let mut line_out = String::new();

    // session.data(1, CryptoVec::from("hello from client!\n".to_string()));

    match tokio::spawn(async move {
        loop {
            tokio::select! {
                res = stream.read_to_string(&mut line_out) => {
                    match res {
                        Err(_) => break,
                        _ => {
                            println!("{}", &line_out);
                            stream.write_all(&line_out.as_bytes()).await;
                        },
                    }
                },
                res = reader.read_line(&mut line_in) => {
                    match res {
                        Err(_) => break,
                        _ => {
                            stream.write_all(&line_in.as_bytes()).await.unwrap();
                        },
                    }
                }
            }
        }
        anyhow::Ok::<()>(())
    })
    .await?
    {
        _ => {}
    }

    // ----
    // let mut channel: Arc<Channel<Msg>> =
    //     Arc::new(self.session.channel_open_session().await.unwrap());
    // channel.exec(false, command).await?;

    // let mut in_stream: ReceiverStream<Vec<u8>> = ReceiverStream::new(stdin);
    // tokio::spawn(async move {
    //     while let Some(item) = in_stream.next().await {
    //         match str::from_utf8(&item) {
    //             Ok(v) => channel.exec(false, v).await.unwrap(),
    //             Err(_) => { /* Not handled :) */ }
    //         };
    //     }
    // });

    // while let Some(msg) = channel.wait().await {
    //     match msg {
    //         russh::ChannelMsg::Data { ref data } => {
    //             let output = Vec::new();
    //             output.write_all(data).unwrap();
    //             self.stdout.send(output).await.unwrap();
    //         }
    //         _ => {}
    //     }
    // }
    // Ok(())
    // ---
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
}

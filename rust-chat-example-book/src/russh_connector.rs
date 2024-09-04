use std::collections::HashMap;
use std::io;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use russh::keys::*;
use russh::server::{Msg, Server as _, Session};
use russh::*;
use server::Config;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, ToSocketAddrs};
use tokio::sync::Mutex;
use tokio::time::sleep;

use crate::utils::{spawn_and_log_error, BoxedResult};

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

    async fn post(&mut self, data: CryptoVec) {
        let mut clients = self.clients.lock().await;
        for ((id, channel), ref mut s) in clients.iter_mut() {
            if *id != self.id {
                let _ = s.data(*channel, data.clone()).await;
            }
        }
    }
}

impl server::Server for Server {
    type Handler = Self;
    fn new_client(&mut self, _: Option<std::net::SocketAddr>) -> Self {
        let s = self.clone();
        self.id += 1;
        println!("Client joined...");
        s
    }
}

#[async_trait]
impl server::Handler for Server {
    type Error = anyhow::Error;

    async fn signal(
        &mut self,
        channel: ChannelId,
        signal: Sig,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if let Sig::Custom(string) = signal {
            println!("{string} from channel id {channel}");
        }
        Ok(())
    }

    async fn channel_open_confirmation(
        &mut self,
        id: ChannelId,
        _max_packet_size: u32,
        _window_size: u32,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        let session_handle = Arc::new(Mutex::new(session.handle()));
        let _ = session_handle
            .clone()
            .lock()
            .await
            .data(id, CryptoVec::from("channel_open_session!\n".to_string()))
            .await;
        tokio::spawn(async move {
            loop {
                let _ = sleep(Duration::from_secs(2));
                println!("2 seconds passed.");
                let data = CryptoVec::from("hello from the server every 2 seconds!\n".to_string());
                let _ = session_handle.clone().lock().await.data(id, data).await;
            }
        });
        Ok(())
    }

    async fn channel_open_session(
        &mut self,
        mut channel: Channel<Msg>,
        session: &mut Session,
    ) -> Result<bool, Self::Error> {
        let mut clients = self.clients.lock().await;
        clients.insert((self.id, channel.id()), session.handle());

        // let session_handle = Arc::new(Mutex::new(session.handle()));
        let channel_id = channel.id().to_owned();

        println!("joined channel id: {}", &channel_id);

        //     tokio::spawn(async move {
        //         loop {
        //             // stream.read_to_string(&mut received_string);

        //             // println!("stream: {}", received_string);

        //             match channel.wait().await {
        //                 Some(ChannelMsg::Data { data }) => {
        //                     let _ = session_handle
        //                         .clone()
        //                         .lock()
        //                         .await
        //                         .data(channel_id, data)
        //                         .await;
        //                 }
        //                 Some(result) => {
        //                     println!("Result: {:?}", result);
        //                     let _ = session_handle
        //                         .clone()
        //                         .lock()
        //                         .await
        //                         .data(channel_id, CryptoVec::from("hello!\n".to_string()))
        //                         .await;
        //                 }
        //                 None => {
        //                     println!("Channel closed");
        //                     return;
        //                 }
        //             }
        //         }
        //     });
        // }

        // tokio::spawn(async move {
        //     let mut buffer = String::new();
        //     let mut stream = channel.into_stream();
        //     stream.write_all("hi!".as_bytes()).await.unwrap();
        //     loop {
        //         tokio::select! {
        //             result = stream.read_to_string(&mut buffer) => {
        //                 match result {
        //                     Ok(0) => break, // client closed connection
        //                     Ok(_n) => {
        //                         println!("received from client: {}", buffer);

        //                         let response = format!("got your input: {}\n", buffer);
        //                         stream.write_all(response.as_bytes()).await.unwrap();
        //                     },
        //                     Err(e) => {
        //                         eprintln!("error reading from channel: {:?}", e);
        //                         break;
        //                     }
        //                 }
        //             }
        //         }
        //     }
        // });

        // session_handle.data(
        //     channel.id(),
        //     CryptoVec::from("channel_open_session".to_string()),
        // );

        // io::stdin().read_line(buf);

        // let mut stream = channel.into_stream();
        // let mut received_string = String::new();
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
        match data {
            // Pressing 'q' closes the connection.
            b"q" => {
                self.clients.lock().await.remove(&(self.id, channel));
                session.close(channel);
            }
            _ => {}
        }

        let data = CryptoVec::from(format!("Got data: {}\r\n", String::from_utf8_lossy(data)));
        self.post(data.clone()).await;

        Ok(())
    }
}

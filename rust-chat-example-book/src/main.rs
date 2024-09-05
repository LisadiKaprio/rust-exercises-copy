use dotenv::dotenv;
use std::env;

mod utils;
use utils::BoxedResult;

mod russh_connector;
use russh_connector::start_russh_server;


// start by cargo run
// then connect from different terminal instances using:    telnet localhost 8080
// or, if connecting to an ssh server, using:               ssh user1@localhost -p 2222

// NOTE:    the code from the book implemented here
//          assumes that you write messages formatted like this:
//          "other_user_1, other_user_2: Hello world!"

#[tokio::main]
pub(crate) async fn main() -> BoxedResult<()> {
    dotenv().ok();

    let host =
        env::var("SERVER_HOST").expect("SERVER_HOST must be named in env file (f.e. 0.0.0.0).");
    let port = env::var("SERVER_PORT").expect("SERVER_PORT must be named in env file (f.e. 2222).");
    let port = port
        .parse::<u16>()
        .expect("SERVER_PORT must be a valid number.");

    start_russh_server((host, port)).await
}

mod utils;
use utils::BoxedResult;

mod telnet_connector;

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
    start_russh_server(("0.0.0.0", 2222)).await

    
    // start_ssh_server(
    //     "WegesrandUser".to_string(),
    //     r"C:\Users\WegesrandUser\.ssh\id_rsa".to_string(),
    //     "run".to_string(),
    // )
    // .await
}

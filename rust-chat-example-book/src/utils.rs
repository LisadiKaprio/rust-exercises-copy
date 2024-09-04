// utils.rs
use std::error::Error;
use std::future::Future;
use std::result::Result;

use tokio::task::JoinHandle;

pub type BoxedResult<T> = Result<T, anyhow::Error>;

pub fn spawn_and_log_error<F>(function: F) -> JoinHandle<()>
where
    F: Future<Output = BoxedResult<()>> + Send + 'static,
{
    tokio::spawn(async move {
        if let Err(e) = function.await {
            eprintln!("{}", e)
        }
    })
}

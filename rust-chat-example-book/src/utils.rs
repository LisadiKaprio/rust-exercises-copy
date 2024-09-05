use std::future::Future;
use std::result::Result;

use rand::Rng;
use tokio::task::JoinHandle;

pub type BoxedResult<T> = Result<T, anyhow::Error>;

pub fn _spawn_and_log_error<F>(function: F) -> JoinHandle<()>
where
    F: Future<Output = BoxedResult<()>> + Send + 'static,
{
    tokio::spawn(async move {
        if let Err(e) = function.await {
            eprintln!("{}", e)
        }
    })
}

pub fn _generate_unique_u32(numbers_already_taken: &Vec<u32>) -> u32 {
    let mut rng = rand::thread_rng();
    let mut new_id = rng.gen::<u32>();

    while numbers_already_taken.contains(&new_id) {
        new_id = rng.gen::<u32>();
    }

    new_id
}

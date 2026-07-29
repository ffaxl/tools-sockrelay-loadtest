use std::time::{Duration, Instant};

use crate::{dial, protocol};

pub struct Config {
    pub target: String,
    pub clients: usize,
    pub duration: Duration,
    pub timeout: Duration,
}

pub struct Sample {
    pub connect: Duration,
    pub request: Duration,
}

#[derive(Default)]
pub struct ClientResult {
    pub samples: Vec<Sample>,
    pub dial_errors: Vec<String>,
    pub request_errors: Vec<String>,
}

/// Runs `config.clients` concurrent tasks, each doing connect → `REQUEST_IDENTITIES` → parse
/// response, in a fresh connection per request (matching real `ssh`/`ssh-add` behavior — the
/// protocol has no multiplexing, one request per connection at a time), until `config.duration`
/// elapses. Every connect and every request individually respects `config.timeout`.
pub async fn run(config: Config) -> Vec<ClientResult> {
    let stop_at = Instant::now() + config.duration;

    let tasks: Vec<_> = (0..config.clients)
        .map(|_| {
            let target = config.target.clone();
            let timeout = config.timeout;
            tokio::spawn(async move { run_one_client(target, timeout, stop_at).await })
        })
        .collect();

    let mut results = Vec::with_capacity(tasks.len());
    for task in tasks {
        results.push(task.await.expect("client task panicked"));
    }
    results
}

async fn run_one_client(target: String, timeout: Duration, stop_at: Instant) -> ClientResult {
    let mut result = ClientResult::default();

    while Instant::now() < stop_at {
        let connect_start = Instant::now();
        let mut conn = match tokio::time::timeout(timeout, dial::dial(&target)).await {
            Ok(Ok(conn)) => conn,
            Ok(Err(e)) => {
                result.dial_errors.push(e.to_string());
                continue;
            }
            Err(_) => {
                result.dial_errors.push("dial timed out".to_string());
                continue;
            }
        };
        let connect = connect_start.elapsed();

        let request_start = Instant::now();
        match tokio::time::timeout(timeout, protocol::request_identities(&mut conn)).await {
            Ok(Ok(_identity_count)) => {
                result.samples.push(Sample {
                    connect,
                    request: request_start.elapsed(),
                });
            }
            Ok(Err(e)) => result.request_errors.push(e.to_string()),
            Err(_) => result.request_errors.push("request timed out".to_string()),
        }
    }

    result
}

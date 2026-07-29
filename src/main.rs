mod dial;
mod harness;
mod protocol;
mod report;

use std::time::{Duration, Instant};

use clap::Parser;

/// Drives concurrent `SSH_AGENTC_REQUEST_IDENTITIES` requests against an ssh-agent-protocol
/// endpoint and reports connect/request latency percentiles — a unix socket on Linux (e.g.
/// sockrelay-client's own socket), or a named pipe on Windows (e.g. the real
/// `\\.\pipe\openssh-ssh-agent`, cross-compiled and run directly via WSL binfmt interop, no
/// separate Windows session needed).
#[derive(Parser)]
struct Args {
    /// Target: a unix socket path (Linux) or a named pipe path (Windows).
    #[arg(long)]
    target: String,

    /// Number of concurrent client tasks.
    #[arg(long, default_value_t = 32)]
    clients: usize,

    /// How long to run for, in seconds.
    #[arg(long = "duration-secs", default_value_t = 15)]
    duration_secs: u64,

    /// Per-operation timeout (connect and request each get their own), in seconds.
    #[arg(long = "timeout-secs", default_value_t = 10)]
    timeout_secs: u64,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    println!(
        "sockrelay-loadtest: target={:?} clients={} duration={}s timeout={}s",
        args.target, args.clients, args.duration_secs, args.timeout_secs
    );

    let config = harness::Config {
        target: args.target,
        clients: args.clients,
        duration: Duration::from_secs(args.duration_secs),
        timeout: Duration::from_secs(args.timeout_secs),
    };

    let wall_start = Instant::now();
    let results = harness::run(config).await;
    let wall_duration = wall_start.elapsed();

    report::print_report(results, wall_duration);
}

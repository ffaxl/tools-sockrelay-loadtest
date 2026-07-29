//! Platform-specific connection to whatever the ssh-agent protocol is being spoken over: a unix
//! socket on Linux (the same kind of path `sockrelay-client` listens on), or a named pipe on
//! Windows (the real agent's own `\\.\pipe\openssh-ssh-agent`, or anything else exposing the same
//! protocol). Only this dial step is platform-specific — `protocol.rs` and `harness.rs` are
//! generic over [`Transport`] and compile/run on both.
//!
//! Deliberately a single attempt, no retry-on-busy: unlike `server/src/pipe.rs`'s production
//! retry logic, this tool's job is to *measure* contention (a real `ERROR_PIPE_BUSY` is a data
//! point, reported as a dial error), not paper over it.

use std::io;

#[cfg(unix)]
pub type Transport = tokio::net::UnixStream;
#[cfg(windows)]
pub type Transport = tokio::net::windows::named_pipe::NamedPipeClient;

#[cfg(unix)]
pub async fn dial(target: &str) -> io::Result<Transport> {
    tokio::net::UnixStream::connect(target).await
}

#[cfg(windows)]
pub async fn dial(target: &str) -> io::Result<Transport> {
    tokio::net::windows::named_pipe::ClientOptions::new().open(target)
}

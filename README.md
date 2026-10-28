# sockrelay-loadtest

Drives concurrent `SSH_AGENTC_REQUEST_IDENTITIES` requests against an ssh-agent-protocol endpoint
and reports connect/request latency percentiles — against a unix socket (e.g.
[`sockrelay-client`](https://github.com/ffaxl/tools-sockrelay)'s own socket, the full relay path)
or directly against a Windows named pipe (e.g. the real `\\.\pipe\openssh-ssh-agent`, to measure
the raw agent's own ceiling with no relay in front of it).

Originally built alongside [`sockrelay`](https://github.com/ffaxl/tools-sockrelay) to measure that
project's behavior under concurrent load; it has no dependency on any of that project's code, so it
lives in its own repository rather than as a workspace member.

```sh
sockrelay-loadtest --target /path/to/agent.sock --clients 32 --duration-secs 15
sockrelay-loadtest --target '\\.\pipe\openssh-ssh-agent' --clients 32 --duration-secs 15
```

Every flag has a sensible default (`--clients 32`, `--duration-secs 15`, `--timeout-secs 10`),
since this is a one-shot operator tool, not a long-running daemon.

## Building

```sh
cargo build --release
```

To run directly against a real Windows named pipe from a WSL shell, cross-compile for the host's
real CPU architecture (WSL's binfmt interop then runs the `.exe` natively, no Wine/emulation):

```sh
rustup target add aarch64-pc-windows-msvc   # or x86_64-pc-windows-msvc on an x86_64 host
cargo install cargo-xwin
cargo xwin build --release --target aarch64-pc-windows-msvc
```

## Testing

```sh
cargo test
cargo clippy --all-targets
```

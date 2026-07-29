//! Minimal ssh-agent wire protocol: just enough to send `SSH_AGENTC_REQUEST_IDENTITIES` and parse
//! `SSH_AGENT_IDENTITIES_ANSWER` back. Message numbers and framing match the
//! draft-miller-ssh-agent protocol (4-byte big-endian length prefix, u32-length-prefixed strings)
//! — same shape as the reference Go tool's `agent.go`, reimplemented here rather than shared,
//! since that tool lives outside this repo.

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

const AGENTC_REQUEST_IDENTITIES: u8 = 11;
const AGENT_FAILURE: u8 = 5;
const AGENT_IDENTITIES_ANSWER: u8 = 12;
/// Mirrors OpenSSH's `AGENT_MAX_LEN`.
const MAX_AGENT_MSG: u32 = 256 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("truncated agent message")]
    Truncated,
    #[error("bad agent frame length {0}")]
    BadFrameLength(u32),
    #[error("agent returned SSH_AGENT_FAILURE")]
    AgentFailure,
    #[error("expected message type {expected}, got {got}")]
    UnexpectedType { expected: u8, got: u8 },
    #[error("implausible key count {0}")]
    ImplausibleKeyCount(u32),
    #[error("{0} trailing bytes in response")]
    TrailingBytes(usize),
}

/// Sends `SSH_AGENTC_REQUEST_IDENTITIES` and returns the number of identities the agent reports.
pub async fn request_identities<T>(io: &mut T) -> Result<u32, ProtocolError>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    let payload = [AGENTC_REQUEST_IDENTITIES];
    io.write_all(&(payload.len() as u32).to_be_bytes()).await?;
    io.write_all(&payload).await?;

    let mut len_buf = [0u8; 4];
    io.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf);
    if len == 0 || len > MAX_AGENT_MSG {
        return Err(ProtocolError::BadFrameLength(len));
    }
    let mut body = vec![0u8; len as usize];
    io.read_exact(&mut body).await?;

    parse_identities_answer(&body)
}

fn parse_identities_answer(body: &[u8]) -> Result<u32, ProtocolError> {
    let mut r = Reader::new(body);
    let msg_type = r.byte()?;
    if msg_type == AGENT_FAILURE {
        return Err(ProtocolError::AgentFailure);
    }
    if msg_type != AGENT_IDENTITIES_ANSWER {
        return Err(ProtocolError::UnexpectedType {
            expected: AGENT_IDENTITIES_ANSWER,
            got: msg_type,
        });
    }

    let count = r.u32()?;
    if count > 4096 {
        return Err(ProtocolError::ImplausibleKeyCount(count));
    }
    for _ in 0..count {
        r.str()?; // key blob
        r.str()?; // comment
    }
    if !r.is_empty() {
        return Err(ProtocolError::TrailingBytes(r.remaining()));
    }
    Ok(count)
}

struct Reader<'a> {
    buf: &'a [u8],
    off: usize,
}

impl<'a> Reader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, off: 0 }
    }

    fn byte(&mut self) -> Result<u8, ProtocolError> {
        let v = *self.buf.get(self.off).ok_or(ProtocolError::Truncated)?;
        self.off += 1;
        Ok(v)
    }

    fn u32(&mut self) -> Result<u32, ProtocolError> {
        let end = self.off.checked_add(4).ok_or(ProtocolError::Truncated)?;
        let bytes = self.buf.get(self.off..end).ok_or(ProtocolError::Truncated)?;
        self.off = end;
        Ok(u32::from_be_bytes(bytes.try_into().unwrap()))
    }

    fn str(&mut self) -> Result<&'a [u8], ProtocolError> {
        let n = self.u32()? as usize;
        let end = self.off.checked_add(n).ok_or(ProtocolError::Truncated)?;
        let v = self.buf.get(self.off..end).ok_or(ProtocolError::Truncated)?;
        self.off = end;
        Ok(v)
    }

    fn is_empty(&self) -> bool {
        self.off == self.buf.len()
    }

    fn remaining(&self) -> usize {
        self.buf.len() - self.off
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identities_answer_frame(count: u32) -> Vec<u8> {
        let mut body = vec![AGENT_IDENTITIES_ANSWER];
        body.extend_from_slice(&count.to_be_bytes());
        for i in 0..count {
            let blob = format!("key-{i}").into_bytes();
            body.extend_from_slice(&(blob.len() as u32).to_be_bytes());
            body.extend_from_slice(&blob);
            let comment = format!("comment-{i}").into_bytes();
            body.extend_from_slice(&(comment.len() as u32).to_be_bytes());
            body.extend_from_slice(&comment);
        }
        let mut framed = (body.len() as u32).to_be_bytes().to_vec();
        framed.extend_from_slice(&body);
        framed
    }

    #[tokio::test]
    async fn request_identities_parses_a_real_answer() {
        let (mut client, mut server) = tokio::io::duplex(4096);
        tokio::spawn(async move {
            // Discard the request frame (length + one type byte) and reply.
            let mut discard = [0u8; 5];
            tokio::io::AsyncReadExt::read_exact(&mut server, &mut discard)
                .await
                .unwrap();
            tokio::io::AsyncWriteExt::write_all(&mut server, &identities_answer_frame(3))
                .await
                .unwrap();
        });

        let count = request_identities(&mut client).await.unwrap();
        assert_eq!(count, 3);
    }

    #[tokio::test]
    async fn request_identities_reports_agent_failure() {
        let (mut client, mut server) = tokio::io::duplex(4096);
        tokio::spawn(async move {
            let mut discard = [0u8; 5];
            tokio::io::AsyncReadExt::read_exact(&mut server, &mut discard)
                .await
                .unwrap();
            let body = [AGENT_FAILURE];
            let mut framed = (body.len() as u32).to_be_bytes().to_vec();
            framed.extend_from_slice(&body);
            tokio::io::AsyncWriteExt::write_all(&mut server, &framed)
                .await
                .unwrap();
        });

        match request_identities(&mut client).await {
            Err(ProtocolError::AgentFailure) => {}
            other => panic!("expected AgentFailure, got {other:?}"),
        }
    }

    #[test]
    fn parse_rejects_trailing_bytes() {
        let mut frame = identities_answer_frame(1);
        frame.push(0xFF); // corrupt: one extra byte past the declared frame
        // Strip the length prefix and reuse the (now-mismatched) body directly.
        let len = u32::from_be_bytes(frame[0..4].try_into().unwrap()) as usize;
        let mut body = frame[4..4 + len].to_vec();
        body.push(0xFF);
        match parse_identities_answer(&body) {
            Err(ProtocolError::TrailingBytes(1)) => {}
            other => panic!("expected TrailingBytes(1), got {other:?}"),
        }
    }
}

//! The hand-off exchange over any byte stream: a little-endian length, the
//! JSON document, one acknowledgement byte back.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _};
use tokio::time::timeout;

use super::HandOff;
use super::error::FrameError;

const LIMIT: u32 = 1 << 20;

const ACKNOWLEDGED: u8 = 0x06;

const EXCHANGE: Duration = Duration::from_secs(5);

pub(super) const CONNECT_WAIT: Duration = Duration::from_secs(3);

pub(super) const CONNECT_STEP: Duration = Duration::from_millis(50);

const fn transport(source: std::io::Error) -> FrameError {
    FrameError::Transport { source }
}

async fn read_hand_off<S>(stream: &mut S) -> Result<HandOff, FrameError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let length = stream.read_u32_le().await.map_err(transport)?;
    if length > LIMIT {
        return Err(FrameError::TooLarge {
            length: u64::from(length),
            limit: LIMIT,
        });
    }
    let mut body = vec![0u8; length as usize];
    stream.read_exact(&mut body).await.map_err(transport)?;
    let hand_off =
        serde_json::from_slice(&body).map_err(|source| FrameError::Malformed { source })?;
    stream.write_u8(ACKNOWLEDGED).await.map_err(transport)?;
    stream.flush().await.map_err(transport)?;
    // Why: a Windows pipe can discard an acknowledgement the launch has not
    // read yet once the server lets go of its end, so the server waits for the
    // launch to close its end first.
    let mut closing = [0u8; 1];
    if let Err(error) = stream.read(&mut closing).await {
        tracing::debug!(error = %error, "second launch closed its end abruptly");
    }
    Ok(hand_off)
}

pub(super) async fn serve<S>(stream: &mut S) -> Result<HandOff, FrameError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    timeout(EXCHANGE, read_hand_off(stream))
        .await
        .unwrap_or(Err(FrameError::TimedOut))
}

async fn write_hand_off<S>(stream: &mut S, length: u32, body: &[u8]) -> Result<(), FrameError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    stream.write_u32_le(length).await.map_err(transport)?;
    stream.write_all(body).await.map_err(transport)?;
    stream.flush().await.map_err(transport)?;
    if stream.read_u8().await.map_err(transport)? == ACKNOWLEDGED {
        Ok(())
    } else {
        Err(FrameError::NotAcknowledged)
    }
}

pub(super) async fn deliver<S>(stream: &mut S, hand_off: &HandOff) -> Result<(), FrameError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let body = serde_json::to_vec(hand_off).map_err(|source| FrameError::Malformed { source })?;
    let Some(length) = u32::try_from(body.len())
        .ok()
        .filter(|length| *length <= LIMIT)
    else {
        return Err(FrameError::TooLarge {
            length: body.len() as u64,
            limit: LIMIT,
        });
    };
    timeout(EXCHANGE, write_hand_off(stream, length, &body))
        .await
        .unwrap_or(Err(FrameError::TimedOut))
}

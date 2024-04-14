use std::{io, io::ErrorKind};

use tokio::{io::AsyncReadExt, net::tcp::OwnedReadHalf};
use valence_protocol::{decode::PacketFrame, PacketDecoder};

pub const PROTO_VERSION: i32 = 763;

pub const READ_BUF_SIZE: usize = 4096;

/// The reader for the connection once handshake is complete.
pub struct IoRead {
    /// The stream of bytes from the client.
    pub stream: OwnedReadHalf,
    /// The decoding buffer and logic
    pub decoder: PacketDecoder,
}

impl IoRead {
    /// Receives a packet from the connection.
    pub async fn recv_packet_raw(&mut self) -> anyhow::Result<PacketFrame> {
        loop {
            if let Some(frame) = self.decoder.try_next_packet()? {
                return Ok(frame);
            }

            self.decoder.reserve(READ_BUF_SIZE);
            let mut buf = self.decoder.take_capacity();

            let bytes_read = self.stream.read_buf(&mut buf).await?;

            if bytes_read == 0 {
                return Err(io::Error::from(ErrorKind::UnexpectedEof).into());
            }

            // This should always be an O(1) unsplit because we reserved space earlier and
            // the call to `read_buf` shouldn't have grown the allocation.
            self.decoder.queue_bytes(buf);
        }
    }
}

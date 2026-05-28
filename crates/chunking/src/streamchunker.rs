//chunking/src/streamchunker.rs
//! Streaming content-defined chunker.
//!
//! Reads from any `Read` source and yields one `Chunk` at a time, so peak
//! memory stays bounded (~`MAX_SIZE`) regardless of file size. This is the
//! streaming counterpart to the in-memory [`crate::FastCDC`] and is guaranteed
//! (by `streaming_matches_in_memory_fastcdc`) to produce identical chunk
//! boundaries, hashes, and offsets — preserving deterministic, cross-machine
//! deduplication.

use crate::Chunk;
use fastcdc::v2020::StreamCDC;
use std::io::Read;

// Must match `FastCDC::new()` defaults so streamed and in-memory chunking agree.
const MIN_SIZE: u32 = 16 * 1024; // 16 KiB
const AVG_SIZE: u32 = 64 * 1024; // 64 KiB
const MAX_SIZE: u32 = 1024 * 1024; // 1 MiB

/// Stream `reader` through FastCDC, yielding chunks lazily. The internal buffer
/// never exceeds `MAX_SIZE`, so chunking a 50 GB file uses ~1 MB of memory.
pub fn stream_chunk_cdc<R: Read>(reader: R) -> impl Iterator<Item = std::io::Result<Chunk>> {
    StreamCDC::new(reader, MIN_SIZE, AVG_SIZE, MAX_SIZE).map(|res| {
        res.map(|cd| Chunk::new(cd.data, cd.offset))
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Chunker, FastCDC};

    /// Deterministic pseudo-random bytes (xorshift64) so the test is reproducible
    /// and large enough to span many content-defined chunks.
    fn pseudo(n: usize) -> Vec<u8> {
        let mut v = Vec::with_capacity(n);
        let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
        for _ in 0..n {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            v.push((x & 0xFF) as u8);
        }
        v
    }

    /// The streaming chunker MUST produce byte-identical chunks (hash, offset,
    /// length) to the in-memory FastCDC. If it diverges, determinism and
    /// cross-machine deduplication break — the core promise of SDAL.
    #[test]
    fn streaming_matches_in_memory_fastcdc() {
        let data = pseudo(3 * 1024 * 1024); // 3 MiB
        let in_mem = FastCDC::new().chunk(&data).unwrap();
        let streamed: Vec<Chunk> = stream_chunk_cdc(&data[..])
            .collect::<std::io::Result<_>>()
            .unwrap();

        assert!(in_mem.len() > 1, "test data should span multiple chunks");
        assert_eq!(streamed.len(), in_mem.len(), "chunk count must match");
        for (s, m) in streamed.iter().zip(in_mem.iter()) {
            assert_eq!(s.hash, m.hash, "chunk hash diverged at offset {}", m.offset);
            assert_eq!(s.offset, m.offset, "chunk offset diverged");
            assert_eq!(s.data.len(), m.data.len(), "chunk length diverged");
        }
    }

    #[test]
    fn streaming_empty_input_yields_no_chunks() {
        let streamed: Vec<Chunk> = stream_chunk_cdc(&b""[..])
            .collect::<std::io::Result<_>>()
            .unwrap();
        assert!(streamed.is_empty());
    }
}

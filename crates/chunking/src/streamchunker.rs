//chunking/src/streamchunker.rs

use crate::Chunk;
use sha2::{Digest, Sha256};
use std::io::Read;

pub fn stream_chunk<R: Read>(mut r: R, s: usize) -> std::io::Result<Vec<Chunk>> {
    let mut v = Vec::new();
    let mut o: u64 = 0;

    loop {
        let mut b = vec![0u8; s];
        let n = r.read(&mut b)?;

        if n == 0 {
            break;
        }

        b.truncate(n);

        let mut h = Sha256::new();
        h.update(&b);
        let x = hex::encode(h.finalize());

        v.push(Chunk {
            data: b,
            hash: x,
            offset: o,
        });

        o += n as u64;
    }

    Ok(v)
}

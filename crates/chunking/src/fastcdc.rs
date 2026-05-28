// chunking/src/fastcdc.rs
// FastCDC wrapper for SDAL with deterministic configuration

use crate::{Chunk, Chunker, ChunkingError};
use fastcdc::v2020::FastCDC as FastCDCImpl;

/// FastCDC chunker with deterministic parameters
///
/// Default configuration:
/// - min_size: 16 KB (16384 bytes)
/// - avg_size: 64 KB (65536 bytes)  
/// - max_size: 1 MB (1048576 bytes)
pub struct FastCDC {
    min_size: usize,
    avg_size: usize,
    max_size: usize,
}

impl FastCDC {
    /// Create a new FastCDC chunker with default parameters
    ///
    /// These parameters MUST remain consistent across all machines
    /// to ensure deterministic chunking and blob hash integrity.
    pub fn new() -> Self {
        Self {
            min_size: 16 * 1024,   // 16 KB
            avg_size: 64 * 1024,   // 64 KB
            max_size: 1024 * 1024, // 1 MB
        }
    }

    /// Create FastCDC with custom parameters
    ///
    /// WARNING: Changing these parameters will result in different chunk boundaries
    /// and different blob hashes. Only use this if you know what you're doing.
    pub fn with_sizes(min_size: usize, avg_size: usize, max_size: usize) -> Self {
        assert!(min_size < avg_size, "min_size must be less than avg_size");
        assert!(avg_size < max_size, "avg_size must be less than max_size");

        Self {
            min_size,
            avg_size,
            max_size,
        }
    }

    /// Get the configuration parameters
    pub fn params(&self) -> (usize, usize, usize) {
        (self.min_size, self.avg_size, self.max_size)
    }
}

impl Default for FastCDC {
    fn default() -> Self {
        Self::new()
    }
}

impl Chunker for FastCDC {
    fn chunk(&self, data: &[u8]) -> Result<Vec<Chunk>, ChunkingError> {
        // Empty data produces no chunks
        if data.is_empty() {
            return Ok(Vec::new());
        }

        let mut chunks = Vec::new();

        // Use fastcdc crate for actual chunking (API requires u32)
        let chunker = FastCDCImpl::new(
            data,
            self.min_size as u32,
            self.avg_size as u32,
            self.max_size as u32,
        );

        for entry in chunker {
            let chunk_data = &data[entry.offset..entry.offset + entry.length];
            let chunk = Chunk::new(chunk_data.to_vec(), entry.offset as u64);
            chunks.push(chunk);
        }

        Ok(chunks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fastcdc_deterministic() {
        let data = b"Hello, world! This is a test of FastCDC chunking. \
                     It should produce the same chunks every time for the same data. \
                     This is critical for SDAL's integrity model."
            .repeat(100);

        let chunker = FastCDC::new();

        // Chunk the same data twice
        let chunks1 = chunker.chunk(&data).unwrap();
        let chunks2 = chunker.chunk(&data).unwrap();

        // Should produce identical results
        assert_eq!(chunks1.len(), chunks2.len());
        for (c1, c2) in chunks1.iter().zip(chunks2.iter()) {
            assert_eq!(c1.hash, c2.hash);
            assert_eq!(c1.offset, c2.offset);
            assert_eq!(c1.data, c2.data);
        }
    }

    #[test]
    fn test_fastcdc_split_large_file() {
        // Test that FastCDC can handle and chunk large files.
        // Content-defined chunking needs *content variation* to find cut points;
        // a block of identical bytes (all zeros) has none and collapses to a
        // a block of identical bytes (e.g. all zeros) has none and collapses to a
        // single chunk, so use deterministic pseudo-random data (xorshift64).
        let mut data = Vec::with_capacity(200 * 1024);
        let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
        for _ in 0..200 * 1024 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            data.push((x & 0xFF) as u8);
        }

        let chunker = FastCDC::new();
        let chunks = chunker.chunk(&data).unwrap();

        // Should create multiple chunks for 200KB of data
        assert!(
            chunks.len() > 1,
            "FastCDC should split 200KB into multiple chunks"
        );

        // Total size should match
        let total: usize = chunks.iter().map(|c| c.data.len()).sum();
        assert_eq!(
            total,
            data.len(),
            "Total chunk size should equal original data"
        );
    }

    #[test]
    fn test_fastcdc_size_constraints() {
        // Create data larger than max_size
        let data = b"X".repeat(2 * 1024 * 1024); // 2 MB

        let chunker = FastCDC::new();
        let chunks = chunker.chunk(&data).unwrap();

        // Verify size constraints
        for chunk in &chunks {
            let size = chunk.data.len();
            assert!(
                size >= chunker.min_size || chunk.offset + size as u64 == data.len() as u64,
                "Chunk too small: {} bytes",
                size
            );
            assert!(size <= chunker.max_size, "Chunk too large: {} bytes", size);
        }
    }

    #[test]
    fn test_fastcdc_offset_continuity() {
        let data = b"Test data for offset verification. ".repeat(1000);

        let chunker = FastCDC::new();
        let chunks = chunker.chunk(&data).unwrap();

        // Verify offset continuity (SDAL invariant)
        let mut expected_offset = 0u64;
        for chunk in &chunks {
            assert_eq!(
                chunk.offset, expected_offset,
                "Offset discontinuity detected"
            );
            expected_offset += chunk.data.len() as u64;
        }

        // Final offset should equal data length
        assert_eq!(expected_offset, data.len() as u64);
    }
}

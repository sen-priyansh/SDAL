// crates/network/src/wire.rs
//
// Binary wire format for streaming chunks over the network.
//
// Frame format:
//   [type: u8][size: u32 LE][data: size bytes]
//
// Types:
//   0x01 = Chunk data
//   0x02 = Object metadata (commit / tree / blob manifest as JSON)
//   0x03 = Ref update
//   0xFF = End of stream
//
// This format enables true streaming — neither side needs to buffer
// the entire payload in memory.

use std::io::{self, Read, Write};

/// Wire frame types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FrameType {
    Chunk = 0x01,
    Object = 0x02,
    Ref = 0x03,
    End = 0xFF,
}

impl FrameType {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0x01 => Some(Self::Chunk),
            0x02 => Some(Self::Object),
            0x03 => Some(Self::Ref),
            0xFF => Some(Self::End),
            _ => None,
        }
    }
}

/// A single frame on the wire
#[derive(Debug, Clone)]
pub struct Frame {
    pub frame_type: FrameType,
    pub data: Vec<u8>,
}

/// Write a single frame to a writer.
///
/// Format: [type: 1 byte][size: 4 bytes LE][data: size bytes]
pub fn write_frame<W: Write>(w: &mut W, frame: &Frame) -> io::Result<()> {
    w.write_all(&[frame.frame_type as u8])?;
    let size = frame.data.len() as u32;
    w.write_all(&size.to_le_bytes())?;
    w.write_all(&frame.data)?;
    Ok(())
}

/// Write an end-of-stream marker.
pub fn write_end<W: Write>(w: &mut W) -> io::Result<()> {
    write_frame(
        w,
        &Frame {
            frame_type: FrameType::End,
            data: Vec::new(),
        },
    )
}

/// Read a single frame from a reader.
///
/// Returns `None` if the frame type is `End`.
pub fn read_frame<R: Read>(r: &mut R) -> io::Result<Option<Frame>> {
    let mut type_buf = [0u8; 1];
    r.read_exact(&mut type_buf)?;

    let frame_type = FrameType::from_u8(type_buf[0]).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unknown frame type: 0x{:02X}", type_buf[0]),
        )
    })?;

    if frame_type == FrameType::End {
        // Read the size field (should be 0) but discard
        let mut size_buf = [0u8; 4];
        r.read_exact(&mut size_buf)?;
        return Ok(None);
    }

    let mut size_buf = [0u8; 4];
    r.read_exact(&mut size_buf)?;
    let size = u32::from_le_bytes(size_buf) as usize;

    // Safety: cap individual frame at 64 MB to prevent memory exhaustion
    const MAX_FRAME_SIZE: usize = 64 * 1024 * 1024;
    if size > MAX_FRAME_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("frame too large: {} bytes (max {})", size, MAX_FRAME_SIZE),
        ));
    }

    let mut data = vec![0u8; size];
    r.read_exact(&mut data)?;

    Ok(Some(Frame { frame_type, data }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_frame_roundtrip() {
        let frame = Frame {
            frame_type: FrameType::Chunk,
            data: b"hello world".to_vec(),
        };

        let mut buf = Vec::new();
        write_frame(&mut buf, &frame).unwrap();
        write_end(&mut buf).unwrap();

        let mut cursor = Cursor::new(&buf);
        let read_back = read_frame(&mut cursor).unwrap().unwrap();
        assert_eq!(read_back.frame_type, FrameType::Chunk);
        assert_eq!(read_back.data, b"hello world");

        // Next frame should be End
        let end = read_frame(&mut cursor).unwrap();
        assert!(end.is_none());
    }

    #[test]
    fn test_empty_frame() {
        let frame = Frame {
            frame_type: FrameType::Object,
            data: Vec::new(),
        };

        let mut buf = Vec::new();
        write_frame(&mut buf, &frame).unwrap();

        let mut cursor = Cursor::new(&buf);
        let read_back = read_frame(&mut cursor).unwrap().unwrap();
        assert_eq!(read_back.frame_type, FrameType::Object);
        assert!(read_back.data.is_empty());
    }
}

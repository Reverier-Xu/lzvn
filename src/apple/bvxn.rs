use crate::{
  Error, Result, StreamProgress, StreamStatus,
  raw::{self, RawDecoder},
};

const HEADER_LEN: usize = 12;

/// Little-endian magic value for an Apple `bvxn` block.
pub const BVXN_MAGIC: u32 = 0x6e78_7662;

/// Parsed metadata from an Apple `bvxn` block header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BvxnHeader {
  /// The number of bytes the payload must decode into.
  pub raw_bytes: usize,
  /// The number of bytes in the raw LZVN payload.
  pub payload_bytes: usize,
}

/// Incremental Apple `bvxn` decoder.
#[derive(Debug, Default)]
pub struct BvxnDecoder {
  input: Vec<u8>,
  input_offset: usize,
  input_base: usize,
  header: Option<BvxnHeader>,
  payload_remaining: usize,
  raw: RawDecoder,
  finished_input: bool,
  finished: bool,
  trailing_after_finish: usize,
}

/// Incremental Apple `bvxn` encoder.
#[derive(Debug, Default)]
pub struct BvxnEncoder {
  input: Vec<u8>,
  encoded: Vec<u8>,
  output_offset: usize,
  finished_input: bool,
  prepared: bool,
}

/// Parse the fixed 12-byte header of an Apple `bvxn` block.
///
/// # Examples
///
/// ```rust
/// let block = [
///     b'b', b'v', b'x', b'n',
///     0x05, 0x00, 0x00, 0x00,
///     0x0e, 0x00, 0x00, 0x00,
///     0xe5, b'h', b'e', b'l', b'l', b'o',
///     0x06, 0, 0, 0, 0, 0, 0, 0,
/// ];
///
/// let header = lzvn::parse_bvxn_header(&block)?;
/// assert_eq!(header.raw_bytes, 5);
/// assert_eq!(header.payload_bytes, 14);
/// # Ok::<(), lzvn::Error>(())
/// ```
pub fn parse_header(src: &[u8]) -> Result<BvxnHeader> {
  if src.len() < HEADER_LEN {
    return Err(Error::TruncatedInput {
      position: src.len(),
    });
  }

  let magic = u32::from_le_bytes(src[0..4].try_into().expect("slice has exact length"));
  if magic != BVXN_MAGIC {
    return Err(Error::UnsupportedBlockMagic { magic });
  }

  let raw_bytes = u32::from_le_bytes(src[4..8].try_into().expect("slice has exact length"));
  let payload_bytes = u32::from_le_bytes(src[8..12].try_into().expect("slice has exact length"));

  Ok(BvxnHeader {
    raw_bytes: raw_bytes as usize,
    payload_bytes: payload_bytes as usize,
  })
}

/// Encode bytes as an Apple `bvxn` block.
///
/// # Examples
///
/// ```rust
/// let block = lzvn::apple::bvxn::encode(b"hello")?;
/// let decoded = lzvn::apple::bvxn::decode(&block)?;
///
/// assert_eq!(decoded, b"hello");
/// # Ok::<(), lzvn::Error>(())
/// ```
pub fn encode(src: &[u8]) -> Result<Vec<u8>> {
  if src.len() > u32::MAX as usize {
    return Err(Error::SizeOverflow {
      value: src.len(),
      max: u32::MAX as usize,
      what: "raw block",
    });
  }

  let payload = raw::encode(src);
  if payload.len() > u32::MAX as usize {
    return Err(Error::SizeOverflow {
      value: payload.len(),
      max: u32::MAX as usize,
      what: "bvxn payload",
    });
  }

  let mut block = Vec::with_capacity(HEADER_LEN + payload.len());
  block.extend_from_slice(&BVXN_MAGIC.to_le_bytes());
  block.extend_from_slice(&(src.len() as u32).to_le_bytes());
  block.extend_from_slice(&(payload.len() as u32).to_le_bytes());
  block.extend_from_slice(&payload);
  Ok(block)
}

impl BvxnEncoder {
  /// Create a new incremental `bvxn` encoder.
  pub fn new() -> Self {
    Self::default()
  }

  /// Append more source bytes.
  pub fn feed(&mut self, input: &[u8]) {
    if !input.is_empty() {
      self.input.extend_from_slice(input);
    }
  }

  /// Mark the source stream as complete.
  pub fn finish_input(&mut self) {
    self.finished_input = true;
  }

  /// Encode into `dst`, returning how many bytes were written and what is
  /// needed next.
  pub fn encode_into(&mut self, dst: &mut [u8]) -> Result<StreamProgress> {
    if !self.prepared {
      if !self.finished_input {
        return Ok(StreamProgress::new(0, StreamStatus::NeedInput));
      }

      self.encoded = encode(&self.input)?;
      self.prepared = true;
    }

    if self.output_offset == self.encoded.len() {
      return Ok(StreamProgress::new(0, StreamStatus::Finished));
    }

    let remaining = self.encoded.len() - self.output_offset;
    let written = remaining.min(dst.len());
    dst[..written].copy_from_slice(&self.encoded[self.output_offset..self.output_offset + written]);
    self.output_offset += written;

    let status = if self.output_offset == self.encoded.len() {
      StreamStatus::Finished
    } else {
      StreamStatus::NeedOutput
    };

    Ok(StreamProgress::new(written, status))
  }

  /// Return whether the encoded block has been fully drained.
  pub fn is_finished(&self) -> bool {
    self.prepared && self.output_offset == self.encoded.len()
  }
}

impl BvxnDecoder {
  /// Create a new incremental `bvxn` decoder.
  pub fn new() -> Self {
    Self::default()
  }

  /// Append more `bvxn` bytes.
  pub fn feed(&mut self, input: &[u8]) {
    if input.is_empty() {
      return;
    }

    if self.finished {
      self.trailing_after_finish += input.len();
      return;
    }

    self.input.extend_from_slice(input);
  }

  /// Mark the `bvxn` input stream as complete.
  pub fn finish_input(&mut self) {
    self.finished_input = true;
  }

  /// Decode into `dst`, returning how many bytes were written and what is
  /// needed next.
  pub fn decode_into(&mut self, dst: &mut [u8]) -> Result<StreamProgress> {
    if self.trailing_after_finish != 0 {
      return Err(Error::TrailingData {
        remaining: self.trailing_after_finish,
      });
    }

    if self.finished {
      return Ok(StreamProgress::new(0, StreamStatus::Finished));
    }

    if self.header.is_none() {
      if self.available_input() < HEADER_LEN {
        if self.finished_input {
          return Err(Error::TruncatedInput {
            position: self.absolute_offset(),
          });
        }
        return Ok(StreamProgress::new(0, StreamStatus::NeedInput));
      }

      let header = parse_header(&self.input[self.input_offset..self.input_offset + HEADER_LEN])?;
      self.header = Some(header);
      self.payload_remaining = header.payload_bytes;
      self.input_offset += HEADER_LEN;
      self.compact_input_if_necessary();
    }

    self.pump_payload()?;
    let progress = self.raw.decode_into(dst)?;
    let expected = self.header.expect("header parsed before decode").raw_bytes;
    let actual = self.raw.total_output();
    if actual > expected {
      return Err(Error::SizeMismatch { expected, actual });
    }

    match progress.status {
      StreamStatus::NeedInput => {
        if self.payload_remaining != 0 && self.finished_input && self.available_input() == 0 {
          return Err(Error::TruncatedInput {
            position: self.absolute_offset(),
          });
        }

        Ok(progress)
      }
      StreamStatus::NeedOutput => Ok(progress),
      StreamStatus::Finished => {
        if actual != expected {
          return Err(Error::SizeMismatch { expected, actual });
        }

        self.finished = true;
        if self.available_input() != 0 {
          return Err(Error::TrailingData {
            remaining: self.available_input(),
          });
        }

        Ok(progress)
      }
    }
  }

  /// Return whether the block has fully decoded.
  pub fn is_finished(&self) -> bool {
    self.finished
  }
}

/// Decode an Apple `bvxn` block into a newly allocated buffer.
///
/// # Examples
///
/// ```rust
/// let block = [
///     b'b', b'v', b'x', b'n',
///     0x05, 0x00, 0x00, 0x00,
///     0x0e, 0x00, 0x00, 0x00,
///     0xe5, b'h', b'e', b'l', b'l', b'o',
///     0x06, 0, 0, 0, 0, 0, 0, 0,
/// ];
///
/// let decoded = lzvn::apple::bvxn::decode(&block)?;
/// assert_eq!(decoded, b"hello");
/// # Ok::<(), lzvn::Error>(())
/// ```
pub fn decode(src: &[u8]) -> Result<Vec<u8>> {
  let (header, payload) = split_block(src)?;
  raw::decode(payload, header.raw_bytes)
}

/// Decode an Apple `bvxn` block into an existing output buffer.
///
/// Returns the number of bytes written.
///
/// # Examples
///
/// ```rust
/// let block = [
///     b'b', b'v', b'x', b'n',
///     0x05, 0x00, 0x00, 0x00,
///     0x0e, 0x00, 0x00, 0x00,
///     0xe5, b'h', b'e', b'l', b'l', b'o',
///     0x06, 0, 0, 0, 0, 0, 0, 0,
/// ];
/// let mut decoded = [0_u8; 5];
///
/// let written = lzvn::apple::bvxn::decode_into(&block, &mut decoded)?;
/// assert_eq!(written, 5);
/// assert_eq!(&decoded[..written], b"hello");
/// # Ok::<(), lzvn::Error>(())
/// ```
pub fn decode_into(src: &[u8], dst: &mut [u8]) -> Result<usize> {
  let (header, payload) = split_block(src)?;
  let written = raw::decode_into(payload, dst)?;
  if written != header.raw_bytes {
    return Err(Error::SizeMismatch {
      expected: header.raw_bytes,
      actual: written,
    });
  }

  Ok(written)
}

fn split_block(src: &[u8]) -> Result<(BvxnHeader, &[u8])> {
  let header = parse_header(src)?;
  let total_len = HEADER_LEN + header.payload_bytes;

  if src.len() < total_len {
    return Err(Error::TruncatedInput {
      position: src.len(),
    });
  }

  if src.len() > total_len {
    return Err(Error::TrailingData {
      remaining: src.len() - total_len,
    });
  }

  Ok((header, &src[HEADER_LEN..total_len]))
}

impl BvxnDecoder {
  fn pump_payload(&mut self) -> Result<()> {
    let to_feed = self.available_input().min(self.payload_remaining);
    if to_feed != 0 {
      let start = self.input_offset;
      let end = start + to_feed;
      self.raw.feed(&self.input[start..end]);
      self.input_offset = end;
      self.payload_remaining -= to_feed;
      self.compact_input_if_necessary();
    }

    if self.payload_remaining == 0 {
      self.raw.finish_input();
      if self.available_input() != 0 {
        return Err(Error::TrailingData {
          remaining: self.available_input(),
        });
      }
    }

    Ok(())
  }

  fn available_input(&self) -> usize {
    self.input.len().saturating_sub(self.input_offset)
  }

  fn absolute_offset(&self) -> usize {
    self.input_base + self.input_offset
  }

  fn compact_input_if_necessary(&mut self) {
    if self.input_offset != 0
      && (self.input_offset == self.input.len() || self.input_offset * 2 >= self.input.len())
    {
      self.input.drain(..self.input_offset);
      self.input_base += self.input_offset;
      self.input_offset = 0;
    }
  }
}

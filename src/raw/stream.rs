use super::{
  encode,
  opcode::{OpcodeKind, classify},
};
use crate::{Error, Result, StreamProgress, StreamStatus};

const MAX_EOS_PADDING: usize = 7;
const INPUT_COMPACT_THRESHOLD: usize = 4_096;

/// Incremental raw LZVN decoder.
///
/// The decoder accepts input in chunks via [`RawDecoder::feed`] and emits
/// decoded bytes into caller-provided buffers via [`RawDecoder::decode_into`].
/// It retains decoded history internally so that back-references continue to
/// work across output boundaries.
#[derive(Debug, Default)]
pub struct RawDecoder {
  input: Vec<u8>,
  input_offset: usize,
  input_base: usize,
  history: Vec<u8>,
  previous_distance: usize,
  pending: Pending,
  finished_input: bool,
  finished: bool,
  eos_padding: usize,
  trailing_after_finish: usize,
}

/// Incremental raw LZVN encoder.
///
/// The encoder buffers all source bytes until [`RawEncoder::finish_input`] is
/// called, then drains the encoded stream in chunks via
/// [`RawEncoder::encode_into`]. This preserves the current whole-buffer match
/// finder while offering a resumable public API.
#[derive(Debug, Default)]
pub struct RawEncoder {
  input: Vec<u8>,
  encoded: Vec<u8>,
  output_offset: usize,
  finished_input: bool,
  prepared: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct Pending {
  literal: usize,
  r#match: usize,
  distance: usize,
}

impl Pending {
  const fn is_empty(self) -> bool {
    self.literal == 0 && self.r#match == 0
  }
}

impl RawDecoder {
  /// Create a new incremental raw decoder.
  pub fn new() -> Self {
    Self::default()
  }

  /// Append more encoded input bytes.
  pub fn feed(&mut self, input: &[u8]) {
    if input.is_empty() {
      return;
    }

    if self.finished {
      if input.iter().all(|byte| *byte == 0)
        && self.eos_padding.saturating_add(input.len()) <= MAX_EOS_PADDING
      {
        self.eos_padding += input.len();
      } else {
        self.trailing_after_finish += input.len();
      }
      return;
    }

    self.input.extend_from_slice(input);
  }

  /// Mark the encoded input stream as complete.
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

    let mut written = 0;
    loop {
      if written == dst.len() {
        return Ok(StreamProgress::new(written, StreamStatus::NeedOutput));
      }

      if self.pending.literal != 0 {
        written += self.copy_pending_literal(&mut dst[written..]);
        if self.pending.literal != 0 {
          return Ok(StreamProgress::new(written, StreamStatus::NeedOutput));
        }
        continue;
      }

      if self.pending.r#match != 0 {
        written += self.copy_pending_match(&mut dst[written..]);
        if self.pending.r#match != 0 {
          return Ok(StreamProgress::new(written, StreamStatus::NeedOutput));
        }
        self.pending.distance = 0;
        continue;
      }

      if self.available_input() == 0 {
        self.compact_input();
        if self.finished_input {
          return Err(Error::TruncatedInput {
            position: self.absolute_offset(),
          });
        }
        return Ok(StreamProgress::new(written, StreamStatus::NeedInput));
      }

      let opcode = self.input[self.input_offset];
      let Some(kind) = classify(opcode) else {
        return Err(Error::InvalidOpcode {
          position: self.absolute_offset(),
          opcode,
        });
      };

      match kind {
        OpcodeKind::SmallDistance => {
          if !self.ensure_buffered(2)? {
            return Ok(StreamProgress::new(written, StreamStatus::NeedInput));
          }

          let literal_len = (opcode >> 6) as usize;
          if !self.ensure_buffered(2 + literal_len)? {
            return Ok(StreamProgress::new(written, StreamStatus::NeedInput));
          }

          let match_len = ((opcode >> 3) & 0x07) as usize + 3;
          let distance =
            (((opcode & 0x07) as usize) << 8) | self.input[self.input_offset + 1] as usize;
          self.prepare_literal_and_match(2, literal_len, match_len, distance)?;
        }
        OpcodeKind::MediumDistance => {
          if !self.ensure_buffered(3)? {
            return Ok(StreamProgress::new(written, StreamStatus::NeedInput));
          }

          let literal_len = ((opcode >> 3) & 0x03) as usize;
          if !self.ensure_buffered(3 + literal_len)? {
            return Ok(StreamProgress::new(written, StreamStatus::NeedInput));
          }

          let second = self.input[self.input_offset + 1];
          let third = self.input[self.input_offset + 2];
          let match_len = ((((opcode & 0x07) as usize) << 2) | (second as usize & 0x03)) + 3;
          let distance = ((second as usize) >> 2) | ((third as usize) << 6);
          self.prepare_literal_and_match(3, literal_len, match_len, distance)?;
        }
        OpcodeKind::LargeDistance => {
          if !self.ensure_buffered(3)? {
            return Ok(StreamProgress::new(written, StreamStatus::NeedInput));
          }

          let literal_len = (opcode >> 6) as usize;
          if !self.ensure_buffered(3 + literal_len)? {
            return Ok(StreamProgress::new(written, StreamStatus::NeedInput));
          }

          let match_len = ((opcode >> 3) & 0x07) as usize + 3;
          let distance = u16::from_le_bytes([
            self.input[self.input_offset + 1],
            self.input[self.input_offset + 2],
          ]) as usize;
          self.prepare_literal_and_match(3, literal_len, match_len, distance)?;
        }
        OpcodeKind::PreviousDistance => {
          let literal_len = (opcode >> 6) as usize;
          if !self.ensure_buffered(1 + literal_len)? {
            return Ok(StreamProgress::new(written, StreamStatus::NeedInput));
          }

          let match_len = ((opcode >> 3) & 0x07) as usize + 3;
          self.prepare_literal_and_match(1, literal_len, match_len, self.previous_distance)?;
        }
        OpcodeKind::SmallLiteral => {
          let literal_len = (opcode & 0x0F) as usize;
          if !self.ensure_buffered(1 + literal_len)? {
            return Ok(StreamProgress::new(written, StreamStatus::NeedInput));
          }

          self.prepare_literal_only(1, literal_len);
        }
        OpcodeKind::LargeLiteral => {
          if !self.ensure_buffered(2)? {
            return Ok(StreamProgress::new(written, StreamStatus::NeedInput));
          }

          let literal_len = self.input[self.input_offset + 1] as usize + 16;
          if !self.ensure_buffered(2 + literal_len)? {
            return Ok(StreamProgress::new(written, StreamStatus::NeedInput));
          }

          self.prepare_literal_only(2, literal_len);
        }
        OpcodeKind::SmallMatch => {
          self.prepare_match_only(1, (opcode & 0x0F) as usize, self.previous_distance)?;
        }
        OpcodeKind::LargeMatch => {
          if !self.ensure_buffered(2)? {
            return Ok(StreamProgress::new(written, StreamStatus::NeedInput));
          }

          let match_len = self.input[self.input_offset + 1] as usize + 16;
          self.prepare_match_only(2, match_len, self.previous_distance)?;
        }
        OpcodeKind::Nop => {
          self.input_offset += 1;
          self.compact_input_if_necessary();
        }
        OpcodeKind::Eos => {
          self.consume_eos()?;

          self.finished = true;
          return Ok(StreamProgress::new(written, StreamStatus::Finished));
        }
      }
    }
  }

  /// Return the total number of decoded bytes produced so far.
  pub fn total_output(&self) -> usize {
    self.history.len()
  }

  /// Return whether the end-of-stream marker has been decoded.
  pub fn is_finished(&self) -> bool {
    self.finished
  }

  fn prepare_literal_only(&mut self, opcode_len: usize, literal_len: usize) {
    self.input_offset += opcode_len;
    self.pending = Pending {
      literal: literal_len,
      r#match: 0,
      distance: 0,
    };
  }

  fn prepare_match_only(
    &mut self, opcode_len: usize, match_len: usize, distance: usize,
  ) -> Result<()> {
    self.validate_distance(distance, self.history.len())?;
    self.input_offset += opcode_len;
    self.previous_distance = distance;
    self.pending = Pending {
      literal: 0,
      r#match: match_len,
      distance,
    };
    self.compact_input_if_necessary();
    Ok(())
  }

  fn prepare_literal_and_match(
    &mut self, opcode_len: usize, literal_len: usize, match_len: usize, distance: usize,
  ) -> Result<()> {
    self.validate_distance(distance, self.history.len() + literal_len)?;
    self.input_offset += opcode_len;
    self.previous_distance = distance;
    self.pending = Pending {
      literal: literal_len,
      r#match: match_len,
      distance,
    };
    Ok(())
  }

  fn copy_pending_literal(&mut self, dst: &mut [u8]) -> usize {
    let count = self.pending.literal.min(dst.len());
    let start = self.input_offset;
    let end = start + count;
    let bytes = &self.input[start..end];
    dst[..count].copy_from_slice(bytes);
    self.history.extend_from_slice(bytes);
    self.input_offset = end;
    self.pending.literal -= count;
    self.compact_input_if_necessary();
    count
  }

  fn copy_pending_match(&mut self, dst: &mut [u8]) -> usize {
    let count = self.pending.r#match.min(dst.len());
    for slot in &mut dst[..count] {
      let index = self.history.len() - self.pending.distance;
      let byte = self.history[index];
      self.history.push(byte);
      *slot = byte;
    }
    self.pending.r#match -= count;
    count
  }

  fn validate_distance(&self, distance: usize, available: usize) -> Result<()> {
    if distance == 0 || distance > available {
      return Err(Error::InvalidMatchDistance {
        distance,
        available,
      });
    }
    Ok(())
  }

  fn ensure_buffered(&self, needed: usize) -> Result<bool> {
    if self.available_input() >= needed {
      return Ok(true);
    }

    if self.finished_input {
      return Err(Error::TruncatedInput {
        position: self.absolute_offset(),
      });
    }

    Ok(false)
  }

  fn available_input(&self) -> usize {
    self.input.len().saturating_sub(self.input_offset)
  }

  fn absolute_offset(&self) -> usize {
    self.input_base + self.input_offset
  }

  fn compact_input_if_necessary(&mut self) {
    if self.pending.is_empty()
      && (self.input_offset == self.input.len()
        || self.input_offset >= INPUT_COMPACT_THRESHOLD
        || self.input_offset * 2 >= self.input.len())
    {
      self.compact_input();
    }
  }

  fn compact_input(&mut self) {
    if self.input_offset == 0 {
      return;
    }

    self.input.drain(..self.input_offset);
    self.input_base += self.input_offset;
    self.input_offset = 0;
  }

  fn consume_eos(&mut self) -> Result<()> {
    self.input_offset += 1;
    let padding = &self.input[self.input_offset..];
    if padding.iter().any(|byte| *byte != 0) {
      return Err(Error::TrailingData {
        remaining: padding.len(),
      });
    }

    if padding.len() > MAX_EOS_PADDING {
      return Err(Error::TrailingData {
        remaining: padding.len() - MAX_EOS_PADDING,
      });
    }

    self.eos_padding = padding.len();
    self.input_offset = self.input.len();
    self.compact_input();
    Ok(())
  }
}

impl RawEncoder {
  /// Create a new incremental raw encoder.
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

      self.encoded = encode(&self.input);
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

  /// Return whether the encoded output has been fully drained.
  pub fn is_finished(&self) -> bool {
    self.prepared && self.output_offset == self.encoded.len()
  }
}

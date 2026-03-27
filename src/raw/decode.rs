use crate::{Error, Result};

use super::opcode::{OpcodeKind, classify};

const MAX_EOS_PADDING: usize = 7;

/// Decode a raw LZVN stream into a newly allocated buffer.
///
/// # Examples
///
/// ```rust
/// let encoded = [
///     0xe5, b'h', b'e', b'l', b'l', b'o',
///     0x06, 0, 0, 0, 0, 0, 0, 0,
/// ];
///
/// let decoded = lzvn::raw::decode(&encoded, 5)?;
/// assert_eq!(decoded, b"hello");
/// # Ok::<(), lzvn::Error>(())
/// ```
pub fn decode(src: &[u8], decoded_len: usize) -> Result<Vec<u8>> {
  let mut dst = vec![0; decoded_len];
  let written = decode_into(src, &mut dst)?;
  if written != decoded_len {
    return Err(Error::SizeMismatch {
      expected: decoded_len,
      actual: written,
    });
  }

  Ok(dst)
}

/// Decode a raw LZVN stream into an existing output buffer.
///
/// The function expects the full raw stream, including its end marker. The
/// decoder accepts both a compact single-byte `0x06` end marker and the padded
/// 8-byte form emitted by Apple-compatible encoders.
/// It returns the number of bytes written before the marker.
///
/// # Examples
///
/// ```rust
/// let encoded = [
///     0xe5, b'h', b'e', b'l', b'l', b'o',
///     0x06, 0, 0, 0, 0, 0, 0, 0,
/// ];
/// let mut decoded = [0_u8; 5];
///
/// let written = lzvn::raw::decode_into(&encoded, &mut decoded)?;
/// assert_eq!(written, 5);
/// assert_eq!(&decoded[..written], b"hello");
/// # Ok::<(), lzvn::Error>(())
/// ```
pub fn decode_into(src: &[u8], dst: &mut [u8]) -> Result<usize> {
  let mut decoder = Decoder {
    src,
    dst,
    input: 0,
    output: 0,
    previous_distance: 0,
  };

  decoder.run()?;
  Ok(decoder.output)
}

struct Decoder<'a> {
  src: &'a [u8],
  dst: &'a mut [u8],
  input: usize,
  output: usize,
  previous_distance: usize,
}

impl Decoder<'_> {
  fn run(&mut self) -> Result<()> {
    loop {
      let opcode = self.byte_at(self.input)?;
      let Some(kind) = classify(opcode) else {
        return Err(Error::InvalidOpcode {
          position: self.input,
          opcode,
        });
      };

      match kind {
        OpcodeKind::SmallDistance => {
          self.require_input(2)?;
          let literal_len = (opcode >> 6) as usize;
          let match_len = ((opcode >> 3) & 0x07) as usize + 3;
          let distance = (((opcode & 0x07) as usize) << 8) | self.src[self.input + 1] as usize;
          self.copy_literal_and_match(2, literal_len, match_len, distance)?;
        }
        OpcodeKind::MediumDistance => {
          self.require_input(3)?;
          let second = self.src[self.input + 1];
          let third = self.src[self.input + 2];
          let literal_len = ((opcode >> 3) & 0x03) as usize;
          let match_len = ((((opcode & 0x07) as usize) << 2) | (second as usize & 0x03)) + 3;
          let distance = ((second as usize) >> 2) | ((third as usize) << 6);
          self.copy_literal_and_match(3, literal_len, match_len, distance)?;
        }
        OpcodeKind::LargeDistance => {
          self.require_input(3)?;
          let literal_len = (opcode >> 6) as usize;
          let match_len = ((opcode >> 3) & 0x07) as usize + 3;
          let distance =
            u16::from_le_bytes([self.src[self.input + 1], self.src[self.input + 2]]) as usize;
          self.copy_literal_and_match(3, literal_len, match_len, distance)?;
        }
        OpcodeKind::PreviousDistance => {
          let literal_len = (opcode >> 6) as usize;
          let match_len = ((opcode >> 3) & 0x07) as usize + 3;
          self.copy_literal_and_match(1, literal_len, match_len, self.previous_distance)?;
        }
        OpcodeKind::SmallLiteral => {
          let literal_len = (opcode & 0x0f) as usize;
          self.copy_literal_only(1, literal_len)?;
        }
        OpcodeKind::LargeLiteral => {
          self.require_input(2)?;
          let literal_len = self.src[self.input + 1] as usize + 16;
          self.copy_literal_only(2, literal_len)?;
        }
        OpcodeKind::SmallMatch => {
          let match_len = (opcode & 0x0f) as usize;
          self.copy_match_only(1, match_len, self.previous_distance)?;
        }
        OpcodeKind::LargeMatch => {
          self.require_input(2)?;
          let match_len = self.src[self.input + 1] as usize + 16;
          self.copy_match_only(2, match_len, self.previous_distance)?;
        }
        OpcodeKind::Nop => {
          self.require_input(1)?;
          self.input += 1;
        }
        OpcodeKind::Eos => {
          self.finish()?;
          return Ok(());
        }
      }
    }
  }

  fn byte_at(&self, position: usize) -> Result<u8> {
    self
      .src
      .get(position)
      .copied()
      .ok_or(Error::TruncatedInput { position })
  }

  fn require_input(&self, needed: usize) -> Result<()> {
    if self.src.len().saturating_sub(self.input) < needed {
      return Err(Error::TruncatedInput {
        position: self.input,
      });
    }

    Ok(())
  }

  fn require_output(&self, needed: usize) -> Result<()> {
    if self.dst.len().saturating_sub(self.output) < needed {
      return Err(Error::OutputTooSmall {
        written: self.output,
        capacity: self.dst.len(),
      });
    }

    Ok(())
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

  fn copy_literal_only(&mut self, opcode_len: usize, literal_len: usize) -> Result<()> {
    self.require_input(opcode_len + literal_len)?;
    self.require_output(literal_len)?;

    let literal_start = self.input + opcode_len;
    let literal_end = literal_start + literal_len;
    self.dst[self.output..self.output + literal_len]
      .copy_from_slice(&self.src[literal_start..literal_end]);

    self.input = literal_end;
    self.output += literal_len;
    Ok(())
  }

  fn copy_match_only(
    &mut self, opcode_len: usize, match_len: usize, distance: usize,
  ) -> Result<()> {
    self.require_input(opcode_len)?;
    self.require_output(match_len)?;
    self.validate_distance(distance, self.output)?;

    self.input += opcode_len;
    self.perform_match(match_len, distance);
    Ok(())
  }

  fn copy_literal_and_match(
    &mut self, opcode_len: usize, literal_len: usize, match_len: usize, distance: usize,
  ) -> Result<()> {
    self.require_input(opcode_len + literal_len)?;
    self.require_output(literal_len + match_len)?;
    self.validate_distance(distance, self.output + literal_len)?;

    let literal_start = self.input + opcode_len;
    let literal_end = literal_start + literal_len;
    self.dst[self.output..self.output + literal_len]
      .copy_from_slice(&self.src[literal_start..literal_end]);

    self.input = literal_end;
    self.output += literal_len;
    self.perform_match(match_len, distance);
    Ok(())
  }

  fn perform_match(&mut self, match_len: usize, distance: usize) {
    let start = self.output;
    for index in 0..match_len {
      let byte = self.dst[start + index - distance];
      self.dst[start + index] = byte;
    }

    self.output += match_len;
    self.previous_distance = distance;
  }

  fn finish(&mut self) -> Result<()> {
    self.require_input(1)?;
    self.input += 1;

    let padding = &self.src[self.input..];
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

    self.input = self.src.len();

    Ok(())
  }
}

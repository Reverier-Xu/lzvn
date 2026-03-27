use crate::{Error, Result, raw};

/// Little-endian magic value for a `decmpfs` xattr header.
pub const DECMPFS_MAGIC: u32 = 0x636D_7066;
/// The size of the fixed `decmpfs` xattr header.
pub const DECMPFS_HEADER_LEN: usize = 16;
/// The maximum size of a `decmpfs` xattr.
pub const DECMPFS_MAX_XATTR_SIZE: usize = 3_802;
/// The maximum number of payload bytes that fit in the xattr after the header.
pub const DECMPFS_MAX_XATTR_DATA_SIZE: usize = DECMPFS_MAX_XATTR_SIZE - DECMPFS_HEADER_LEN;
/// The independent chunk size used by resource-fork-backed LZVN data.
pub const DECMPFS_CHUNK_SIZE: usize = 0x1_0000;

const LZVN_UNCOMPRESSED_SENTINEL: u8 = 0x06;

/// Parsed `decmpfs` compression type values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecmpfsCompressionType {
  /// zlib payload stored in the xattr.
  ZlibXattr,
  /// zlib payload stored in the resource fork.
  ZlibResourceFork,
  /// LZVN payload stored in the xattr.
  LzvnXattr,
  /// LZVN payload stored in the resource fork.
  LzvnResourceFork,
  /// Raw payload stored in the xattr.
  RawXattr,
  /// Raw payload stored in the resource fork.
  RawResourceFork,
  /// LZFSE payload stored in the xattr.
  LzfseXattr,
  /// LZFSE payload stored in the resource fork.
  LzfseResourceFork,
  /// LZBitmap payload stored in the xattr.
  LzBitmapXattr,
  /// LZBitmap payload stored in the resource fork.
  LzBitmapResourceFork,
  /// An unknown compression type value.
  Unknown(u32),
}

impl DecmpfsCompressionType {
  /// Construct a parsed compression type from the raw on-disk value.
  pub const fn from_raw(value: u32) -> Self {
    match value {
      3 => Self::ZlibXattr,
      4 => Self::ZlibResourceFork,
      7 => Self::LzvnXattr,
      8 => Self::LzvnResourceFork,
      9 => Self::RawXattr,
      10 => Self::RawResourceFork,
      11 => Self::LzfseXattr,
      12 => Self::LzfseResourceFork,
      13 => Self::LzBitmapXattr,
      14 => Self::LzBitmapResourceFork,
      other => Self::Unknown(other),
    }
  }

  /// Return the raw on-disk value of the compression type.
  pub const fn raw_value(self) -> u32 {
    match self {
      Self::ZlibXattr => 3,
      Self::ZlibResourceFork => 4,
      Self::LzvnXattr => 7,
      Self::LzvnResourceFork => 8,
      Self::RawXattr => 9,
      Self::RawResourceFork => 10,
      Self::LzfseXattr => 11,
      Self::LzfseResourceFork => 12,
      Self::LzBitmapXattr => 13,
      Self::LzBitmapResourceFork => 14,
      Self::Unknown(value) => value,
    }
  }

  /// Return whether this type stores its payload in the resource fork.
  pub const fn uses_resource_fork(self) -> bool {
    matches!(
      self,
      Self::ZlibResourceFork
        | Self::LzvnResourceFork
        | Self::RawResourceFork
        | Self::LzfseResourceFork
        | Self::LzBitmapResourceFork
    )
  }
}

/// Parsed metadata from a `decmpfs` xattr header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecmpfsHeader {
  /// The compression type declared by the wrapper.
  pub compression_type: DecmpfsCompressionType,
  /// The number of bytes the wrapped payload decodes into.
  pub uncompressed_size: usize,
}

/// Encoded `decmpfs` output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedDecmpfs {
  /// The full `com.apple.decmpfs` xattr value.
  pub xattr: Vec<u8>,
  /// Optional resource fork data when the payload does not fit inline.
  pub resource_fork: Option<Vec<u8>>,
}

/// Parse the fixed 16-byte `decmpfs` header.
///
/// # Examples
///
/// ```rust
/// let xattr = [
///     b'f', b'p', b'm', b'c',
///     0x07, 0x00, 0x00, 0x00,
///     0x05, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
///     0x06, b'h', b'e', b'l', b'l', b'o',
/// ];
///
/// let header = lzvn::parse_decmpfs_header(&xattr)?;
/// assert_eq!(header.compression_type, lzvn::DecmpfsCompressionType::LzvnXattr);
/// assert_eq!(header.uncompressed_size, 5);
/// # Ok::<(), lzvn::Error>(())
/// ```
pub fn parse_header(xattr: &[u8]) -> Result<DecmpfsHeader> {
  if xattr.len() < DECMPFS_HEADER_LEN {
    return Err(Error::TruncatedInput {
      position: xattr.len(),
    });
  }

  let magic = u32::from_le_bytes(xattr[0..4].try_into().expect("slice has exact length"));
  if magic != DECMPFS_MAGIC {
    return Err(Error::InvalidDecmpfsMagic { magic });
  }

  let compression_type =
    u32::from_le_bytes(xattr[4..8].try_into().expect("slice has exact length"));
  let uncompressed_size =
    u64::from_le_bytes(xattr[8..16].try_into().expect("slice has exact length"));
  let uncompressed_size = usize::try_from(uncompressed_size).map_err(|_| Error::SizeOverflow {
    value: usize::MAX,
    max: usize::MAX,
    what: "decmpfs uncompressed size",
  })?;

  Ok(DecmpfsHeader {
    compression_type: DecmpfsCompressionType::from_raw(compression_type),
    uncompressed_size,
  })
}

/// Encode bytes as a `decmpfs` LZVN wrapper.
///
/// Small payloads are stored inline in the xattr; larger payloads are split
/// into 64 KiB resource-fork blocks with an offset table.
///
/// # Examples
///
/// ```rust
/// let encoded = lzvn::encode_decmpfs(b"hello")?;
/// let decoded = lzvn::decode_decmpfs(&encoded.xattr, encoded.resource_fork.as_deref())?;
///
/// assert_eq!(decoded, b"hello");
/// # Ok::<(), lzvn::Error>(())
/// ```
pub fn encode(src: &[u8]) -> Result<EncodedDecmpfs> {
  let inline_payload = best_lzvn_wrapper_payload(src);
  if inline_payload.len() <= DECMPFS_MAX_XATTR_DATA_SIZE {
    return Ok(EncodedDecmpfs {
      xattr: build_xattr(
        DecmpfsCompressionType::LzvnXattr,
        src.len(),
        &inline_payload,
      )?,
      resource_fork: None,
    });
  }

  let resource_fork = encode_resource_fork(src)?;
  Ok(EncodedDecmpfs {
    xattr: build_xattr(DecmpfsCompressionType::LzvnResourceFork, src.len(), &[])?,
    resource_fork: Some(resource_fork),
  })
}

/// Decode a `decmpfs` value using optional resource-fork data.
///
/// # Examples
///
/// ```rust
/// let encoded = lzvn::encode_decmpfs(b"hello")?;
/// let decoded = lzvn::decode_decmpfs(&encoded.xattr, encoded.resource_fork.as_deref())?;
///
/// assert_eq!(decoded, b"hello");
/// # Ok::<(), lzvn::Error>(())
/// ```
pub fn decode(xattr: &[u8], resource_fork: Option<&[u8]>) -> Result<Vec<u8>> {
  let (header, payload) = split_xattr(xattr)?;
  let mut dst = vec![0; header.uncompressed_size];
  let written = decode_body(header, payload, resource_fork, &mut dst)?;
  if written != header.uncompressed_size {
    return Err(Error::SizeMismatch {
      expected: header.uncompressed_size,
      actual: written,
    });
  }

  Ok(dst)
}

/// Decode a `decmpfs` value into an existing buffer.
///
/// Returns the number of bytes written.
pub fn decode_into(xattr: &[u8], resource_fork: Option<&[u8]>, dst: &mut [u8]) -> Result<usize> {
  let (header, payload) = split_xattr(xattr)?;
  if dst.len() < header.uncompressed_size {
    return Err(Error::OutputTooSmall {
      written: 0,
      capacity: dst.len(),
    });
  }

  let written = decode_body(header, payload, resource_fork, dst)?;
  if written != header.uncompressed_size {
    return Err(Error::SizeMismatch {
      expected: header.uncompressed_size,
      actual: written,
    });
  }

  Ok(written)
}

fn split_xattr(xattr: &[u8]) -> Result<(DecmpfsHeader, &[u8])> {
  let header = parse_header(xattr)?;
  Ok((header, &xattr[DECMPFS_HEADER_LEN..]))
}

fn decode_body(
  header: DecmpfsHeader, payload: &[u8], resource_fork: Option<&[u8]>, dst: &mut [u8],
) -> Result<usize> {
  match header.compression_type {
    DecmpfsCompressionType::LzvnXattr => decode_lzvn_block(payload, header.uncompressed_size, dst),
    DecmpfsCompressionType::LzvnResourceFork => {
      if !payload.is_empty() {
        return Err(Error::TrailingData {
          remaining: payload.len(),
        });
      }

      let resource_fork = resource_fork.ok_or(Error::MissingResourceFork)?;
      decode_lzvn_resource_fork(resource_fork, header.uncompressed_size, dst)
    }
    other => Err(Error::UnsupportedDecmpfsCompressionType {
      compression_type: other.raw_value(),
    }),
  }
}

fn decode_lzvn_block(payload: &[u8], expected: usize, dst: &mut [u8]) -> Result<usize> {
  if payload.len() == expected.saturating_add(1)
    && payload.first() == Some(&LZVN_UNCOMPRESSED_SENTINEL)
  {
    dst[..expected].copy_from_slice(&payload[1..]);
    return Ok(expected);
  }

  raw::decode_into(payload, &mut dst[..expected])
}

fn decode_lzvn_resource_fork(
  resource_fork: &[u8], expected: usize, dst: &mut [u8],
) -> Result<usize> {
  let block_count = block_count(expected);
  let table_len = offset_table_len(block_count)?;
  if resource_fork.len() < table_len {
    return Err(Error::InvalidResourceFork {
      reason: "offset table is truncated",
    });
  }

  let first_offset = read_offset(resource_fork, 0)?;
  if first_offset != table_len {
    return Err(Error::InvalidResourceFork {
      reason: "first block offset does not match offset table size",
    });
  }

  if block_count == 0 {
    if first_offset != resource_fork.len() {
      return Err(Error::InvalidResourceFork {
        reason: "empty resource fork has trailing data",
      });
    }
    return Ok(0);
  }

  let mut produced = 0;
  let mut previous_offset = first_offset;
  for block_index in 0..block_count {
    let next_offset = read_offset(resource_fork, block_index + 1)?;
    if next_offset < previous_offset || next_offset > resource_fork.len() {
      return Err(Error::InvalidResourceFork {
        reason: "block offsets are invalid",
      });
    }

    let expected_block = remaining_block_size(expected, block_index);
    let block = &resource_fork[previous_offset..next_offset];
    let written = decode_lzvn_block(
      block,
      expected_block,
      &mut dst[produced..produced + expected_block],
    )?;
    if written != expected_block {
      return Err(Error::SizeMismatch {
        expected: expected_block,
        actual: written,
      });
    }

    produced += expected_block;
    previous_offset = next_offset;
  }

  if previous_offset != resource_fork.len() {
    return Err(Error::InvalidResourceFork {
      reason: "last block offset does not reach end of resource fork",
    });
  }

  Ok(produced)
}

fn build_xattr(
  compression_type: DecmpfsCompressionType, uncompressed_size: usize, payload: &[u8],
) -> Result<Vec<u8>> {
  let uncompressed_size = u64::try_from(uncompressed_size).map_err(|_| Error::SizeOverflow {
    value: uncompressed_size,
    max: u64::MAX as usize,
    what: "decmpfs uncompressed size",
  })?;

  let mut xattr = Vec::with_capacity(DECMPFS_HEADER_LEN + payload.len());
  xattr.extend_from_slice(&DECMPFS_MAGIC.to_le_bytes());
  xattr.extend_from_slice(&compression_type.raw_value().to_le_bytes());
  xattr.extend_from_slice(&uncompressed_size.to_le_bytes());
  xattr.extend_from_slice(payload);
  Ok(xattr)
}

fn encode_resource_fork(src: &[u8]) -> Result<Vec<u8>> {
  let block_count = block_count(src.len());
  let table_len = offset_table_len(block_count)?;
  let mut offsets = Vec::with_capacity(block_count + 1);
  let mut blocks = Vec::with_capacity(block_count);
  let mut offset = u32::try_from(table_len).map_err(|_| Error::SizeOverflow {
    value: table_len,
    max: u32::MAX as usize,
    what: "decmpfs resource fork offset table",
  })?;
  offsets.push(offset);

  for chunk in src.chunks(DECMPFS_CHUNK_SIZE) {
    let block = best_lzvn_wrapper_payload(chunk);
    let block_len = u32::try_from(block.len()).map_err(|_| Error::SizeOverflow {
      value: block.len(),
      max: u32::MAX as usize,
      what: "decmpfs resource block",
    })?;
    offset = offset.checked_add(block_len).ok_or(Error::SizeOverflow {
      value: usize::MAX,
      max: u32::MAX as usize,
      what: "decmpfs resource fork",
    })?;
    offsets.push(offset);
    blocks.push(block);
  }

  let mut resource_fork = Vec::with_capacity(offset as usize);
  for entry in offsets {
    resource_fork.extend_from_slice(&entry.to_le_bytes());
  }
  for block in blocks {
    resource_fork.extend_from_slice(&block);
  }

  Ok(resource_fork)
}

fn best_lzvn_wrapper_payload(src: &[u8]) -> Vec<u8> {
  let compressed = raw::encode(src);
  let sentinel = lzvn_uncompressed_payload(src);
  if sentinel.len() < compressed.len() {
    sentinel
  } else {
    compressed
  }
}

fn lzvn_uncompressed_payload(src: &[u8]) -> Vec<u8> {
  let mut payload = Vec::with_capacity(src.len() + 1);
  payload.push(LZVN_UNCOMPRESSED_SENTINEL);
  payload.extend_from_slice(src);
  payload
}

fn block_count(size: usize) -> usize {
  if size == 0 {
    0
  } else {
    size.div_ceil(DECMPFS_CHUNK_SIZE)
  }
}

fn remaining_block_size(total: usize, block_index: usize) -> usize {
  let start = block_index * DECMPFS_CHUNK_SIZE;
  (total - start).min(DECMPFS_CHUNK_SIZE)
}

fn offset_table_len(block_count: usize) -> Result<usize> {
  let entries = block_count.checked_add(1).ok_or(Error::SizeOverflow {
    value: usize::MAX,
    max: usize::MAX,
    what: "decmpfs resource fork block count",
  })?;
  entries.checked_mul(4).ok_or(Error::SizeOverflow {
    value: usize::MAX,
    max: u32::MAX as usize,
    what: "decmpfs resource fork offset table",
  })
}

fn read_offset(resource_fork: &[u8], index: usize) -> Result<usize> {
  let start = index.checked_mul(4).ok_or(Error::InvalidResourceFork {
    reason: "offset table index overflowed",
  })?;
  let end = start + 4;
  let bytes = resource_fork
    .get(start..end)
    .ok_or(Error::InvalidResourceFork {
      reason: "offset table is truncated",
    })?;
  Ok(u32::from_le_bytes(bytes.try_into().expect("slice has exact length")) as usize)
}

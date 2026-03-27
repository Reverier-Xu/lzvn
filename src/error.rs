use thiserror::Error;

/// Result alias used by this crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors produced while encoding or decoding raw LZVN streams and Apple
/// wrappers such as `bvxn` and `decmpfs`.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum Error {
  /// The input ended before a full instruction or end marker was available.
  #[error("input ended unexpectedly at byte {position}")]
  TruncatedInput {
    /// The byte offset where decoding needed more input.
    position: usize,
  },

  /// The decoder found an opcode that is reserved or undefined.
  #[error("invalid opcode 0x{opcode:02x} at byte {position}")]
  InvalidOpcode {
    /// The byte offset of the invalid opcode.
    position: usize,
    /// The invalid opcode byte.
    opcode: u8,
  },

  /// A match referred to bytes that do not exist in the decoded history.
  #[error(
    "invalid match distance {distance}; only {available} bytes are available for back-references"
  )]
  InvalidMatchDistance {
    /// The decoded distance from the instruction.
    distance: usize,
    /// The number of bytes available for a back-reference.
    available: usize,
  },

  /// The output buffer cannot hold the decoded bytes.
  #[error("output buffer is too small after writing {written} bytes into a buffer of {capacity}")]
  OutputTooSmall {
    /// The number of bytes written before the buffer filled up.
    written: usize,
    /// The total size of the destination buffer.
    capacity: usize,
  },

  /// The decoded byte count does not match the caller's expectation.
  #[error("decoded {actual} bytes but expected {expected}")]
  SizeMismatch {
    /// The caller-provided expected length.
    expected: usize,
    /// The actual decoded length.
    actual: usize,
  },

  /// Data remains after a successful end-of-stream marker.
  #[error("input contains {remaining} trailing bytes after the end-of-stream marker")]
  TrailingData {
    /// The number of trailing bytes.
    remaining: usize,
  },

  /// The Apple block header is well-formed but not a `bvxn` block.
  #[error("unsupported Apple block magic 0x{magic:08x}")]
  UnsupportedBlockMagic {
    /// The 32-bit little-endian block magic.
    magic: u32,
  },

  /// The decmpfs xattr header is well-formed but does not have the expected
  /// magic value.
  #[error("invalid decmpfs magic 0x{magic:08x}")]
  InvalidDecmpfsMagic {
    /// The 32-bit little-endian decmpfs magic.
    magic: u32,
  },

  /// The decmpfs wrapper uses a compression type this crate does not support.
  #[error("unsupported decmpfs compression type {compression_type}")]
  UnsupportedDecmpfsCompressionType {
    /// The raw decmpfs compression type value.
    compression_type: u32,
  },

  /// The decmpfs wrapper requires a resource fork but none was provided.
  #[error("decmpfs resource fork data is required for this compression type")]
  MissingResourceFork,

  /// The decmpfs resource fork layout is malformed.
  #[error("invalid decmpfs resource fork: {reason}")]
  InvalidResourceFork {
    /// A short description of the structural issue.
    reason: &'static str,
  },

  /// A size does not fit inside the target container format.
  #[error("{what} size {value} exceeds the format limit of {max}")]
  SizeOverflow {
    /// The size that could not be represented.
    value: usize,
    /// The maximum representable size.
    max: usize,
    /// A short label describing the overflowing field.
    what: &'static str,
  },
}

#![forbid(unsafe_code)]
//! Safe, clean-room LZVN encoding and decoding primitives.
//!
//! The crate currently provides safe Rust support for raw LZVN streams and
//! Apple wrappers such as `bvxn` and `decmpfs`, with no `unsafe` code.

pub mod apple;
mod error;
pub mod raw;
mod stream;

pub use error::{Error, Result};
pub use raw::{RawDecoder, RawEncoder};
pub use stream::{StreamProgress, StreamStatus};

/// Encode bytes as a raw LZVN stream.
///
/// The returned stream includes the padded 8-byte end-of-stream marker emitted
/// by Apple-compatible encoders.
///
/// # Examples
///
/// ```rust
/// let encoded = lzvn::encode_raw(b"hello");
/// let decoded = lzvn::decode_raw(&encoded, 5)?;
///
/// assert_eq!(decoded, b"hello");
/// # Ok::<(), lzvn::Error>(())
/// ```
pub fn encode_raw(src: &[u8]) -> Vec<u8> {
  raw::encode(src)
}

/// Decode a raw LZVN stream into a newly allocated buffer.
///
/// The caller supplies the expected decoded size because raw LZVN streams do
/// not carry it on their own.
///
/// # Examples
///
/// ```rust
/// let encoded = [
///     0xe5, b'h', b'e', b'l', b'l', b'o',
///     0x06, 0, 0, 0, 0, 0, 0, 0,
/// ];
///
/// let decoded = lzvn::decode_raw(&encoded, 5)?;
/// assert_eq!(decoded, b"hello");
/// # Ok::<(), lzvn::Error>(())
/// ```
pub fn decode_raw(src: &[u8], decoded_len: usize) -> Result<Vec<u8>> {
  raw::decode(src, decoded_len)
}

/// Decode a raw LZVN stream into an existing output buffer.
///
/// Returns the number of bytes written before the end-of-stream marker.
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
/// let written = lzvn::decode_raw_into(&encoded, &mut decoded)?;
/// assert_eq!(written, 5);
/// assert_eq!(&decoded[..written], b"hello");
/// # Ok::<(), lzvn::Error>(())
/// ```
pub fn decode_raw_into(src: &[u8], dst: &mut [u8]) -> Result<usize> {
  raw::decode_into(src, dst)
}

/// Encode bytes as an Apple `decmpfs` LZVN wrapper.
///
/// Small payloads are stored inline in the xattr and larger payloads are
/// stored in a resource fork.
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
pub fn encode_decmpfs(src: &[u8]) -> Result<apple::fs::decmpfs::EncodedDecmpfs> {
  apple::fs::decmpfs::encode(src)
}

/// Decode an Apple `decmpfs` wrapper into a newly allocated buffer.
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
pub fn decode_decmpfs(xattr: &[u8], resource_fork: Option<&[u8]>) -> Result<Vec<u8>> {
  apple::fs::decmpfs::decode(xattr, resource_fork)
}

/// Decode an Apple `decmpfs` wrapper into an existing output buffer.
pub fn decode_decmpfs_into(
  xattr: &[u8], resource_fork: Option<&[u8]>, dst: &mut [u8],
) -> Result<usize> {
  apple::fs::decmpfs::decode_into(xattr, resource_fork, dst)
}

/// Encode bytes as an Apple `bvxn` block.
///
/// # Examples
///
/// ```rust
/// let encoded = lzvn::encode_bvxn(b"hello")?;
/// let decoded = lzvn::decode_bvxn(&encoded)?;
///
/// assert_eq!(decoded, b"hello");
/// # Ok::<(), lzvn::Error>(())
/// ```
pub fn encode_bvxn(src: &[u8]) -> Result<Vec<u8>> {
  apple::bvxn::encode(src)
}

/// Decode an Apple `bvxn` block into a newly allocated buffer.
///
/// # Examples
///
/// ```rust
/// let encoded = [
///     b'b', b'v', b'x', b'n',
///     0x05, 0x00, 0x00, 0x00,
///     0x0e, 0x00, 0x00, 0x00,
///     0xe5, b'h', b'e', b'l', b'l', b'o',
///     0x06, 0, 0, 0, 0, 0, 0, 0,
/// ];
///
/// let decoded = lzvn::decode_bvxn(&encoded)?;
/// assert_eq!(decoded, b"hello");
/// # Ok::<(), lzvn::Error>(())
/// ```
pub fn decode_bvxn(src: &[u8]) -> Result<Vec<u8>> {
  apple::bvxn::decode(src)
}

/// Decode an Apple `bvxn` block into an existing output buffer.
///
/// Returns the number of bytes written.
///
/// # Examples
///
/// ```rust
/// let encoded = [
///     b'b', b'v', b'x', b'n',
///     0x05, 0x00, 0x00, 0x00,
///     0x0e, 0x00, 0x00, 0x00,
///     0xe5, b'h', b'e', b'l', b'l', b'o',
///     0x06, 0, 0, 0, 0, 0, 0, 0,
/// ];
/// let mut decoded = [0_u8; 5];
///
/// let written = lzvn::decode_bvxn_into(&encoded, &mut decoded)?;
/// assert_eq!(written, 5);
/// assert_eq!(&decoded[..written], b"hello");
/// # Ok::<(), lzvn::Error>(())
/// ```
pub fn decode_bvxn_into(src: &[u8], dst: &mut [u8]) -> Result<usize> {
  apple::bvxn::decode_into(src, dst)
}

pub use apple::{
  bvxn::{BVXN_MAGIC, BvxnDecoder, BvxnEncoder, BvxnHeader, parse_header as parse_bvxn_header},
  fs::decmpfs::{
    DECMPFS_CHUNK_SIZE, DECMPFS_HEADER_LEN, DECMPFS_MAGIC, DECMPFS_MAX_XATTR_DATA_SIZE,
    DECMPFS_MAX_XATTR_SIZE, DecmpfsCompressionType, DecmpfsHeader, EncodedDecmpfs,
    parse_header as parse_decmpfs_header,
  },
};

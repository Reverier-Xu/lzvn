//! End-to-end tests against real-world fixture data vendored in
//! `tests/fixtures`.

use lzvn::{DecmpfsCompressionType, decode_decmpfs, decode_raw, parse_decmpfs_header};

const BLACKTOP_LZVN_ENC: &[u8] = include_bytes!("fixtures/bin/blacktop_lzvn_enc.bin");
const BLACKTOP_LZVN_DEC: &[u8] = include_bytes!("fixtures/bin/blacktop_lzvn_dec.bin");
const LIBFSAPFS_LZVN_COMPRESSED: &[u8] =
  include_bytes!("fixtures/bin/libfsapfs_lzvn_compressed.bin");
const LIBFSAPFS_LZVN_EXPECTED: &[u8] = include_bytes!("fixtures/bin/libfsapfs_lzvn_expected.bin");
const LIBFSAPFS_LZVN_UNCOMPRESSED_SENTINEL: &[u8] =
  include_bytes!("fixtures/bin/libfsapfs_lzvn_uncompressed_sentinel.bin");
const LIBFSAPFS_DECMPFS_LZVN_XATTR: &[u8] =
  include_bytes!("fixtures/bin/libfsapfs_decmpfs_lzvn_xattr.bin");

#[test]
fn decodes_real_blacktop_raw_fixture() {
  let decoded =
    decode_raw(BLACKTOP_LZVN_ENC, BLACKTOP_LZVN_DEC.len()).expect("real raw fixture should decode");
  assert_eq!(decoded, BLACKTOP_LZVN_DEC);
}

#[test]
fn decodes_real_libfsapfs_raw_fixture() {
  let decoded = decode_raw(LIBFSAPFS_LZVN_COMPRESSED, LIBFSAPFS_LZVN_EXPECTED.len())
    .expect("libfsapfs raw fixture should decode");
  assert_eq!(decoded, LIBFSAPFS_LZVN_EXPECTED);
}

#[test]
fn decodes_real_libfsapfs_decmpfs_xattr_fixture() {
  let header =
    parse_decmpfs_header(LIBFSAPFS_DECMPFS_LZVN_XATTR).expect("real decmpfs header should parse");
  assert_eq!(header.compression_type, DecmpfsCompressionType::LzvnXattr);
  assert_eq!(header.uncompressed_size, LIBFSAPFS_LZVN_EXPECTED.len());

  let decoded =
    decode_decmpfs(LIBFSAPFS_DECMPFS_LZVN_XATTR, None).expect("real decmpfs fixture should decode");
  assert_eq!(decoded, LIBFSAPFS_LZVN_EXPECTED);
}

#[test]
fn decodes_real_libfsapfs_uncompressed_sentinel_payload_when_wrapped() {
  let mut xattr = Vec::with_capacity(16 + LIBFSAPFS_LZVN_UNCOMPRESSED_SENTINEL.len());
  xattr.extend_from_slice(b"fpmc");
  xattr.extend_from_slice(&7_u32.to_le_bytes());
  xattr.extend_from_slice(&(LIBFSAPFS_LZVN_EXPECTED.len() as u64).to_le_bytes());
  xattr.extend_from_slice(LIBFSAPFS_LZVN_UNCOMPRESSED_SENTINEL);

  let decoded =
    decode_decmpfs(&xattr, None).expect("sentinel payload should decode through decmpfs");
  assert_eq!(decoded, LIBFSAPFS_LZVN_EXPECTED);
}

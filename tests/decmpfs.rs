//! Integration tests for Apple `decmpfs` wrappers.

use lzvn::{
  DecmpfsCompressionType, Error, decode_decmpfs, decode_decmpfs_into, encode_decmpfs,
  parse_decmpfs_header,
};

#[test]
fn parses_decmpfs_header() {
  let xattr = [
    b'f', b'p', b'm', b'c', 0x07, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x06, b'h', b'e', b'l', b'l', b'o',
  ];

  let header = parse_decmpfs_header(&xattr).expect("header should parse");
  assert_eq!(header.compression_type, DecmpfsCompressionType::LzvnXattr);
  assert_eq!(header.uncompressed_size, 5);
}

#[test]
fn encodes_small_input_as_inline_lzvn_wrapper() {
  let encoded = encode_decmpfs(b"hello").expect("inline decmpfs encode should succeed");

  assert_eq!(encoded.resource_fork, None);
  assert_eq!(
    encoded.xattr,
    [
      b'f', b'p', b'm', b'c', 0x07, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
      0x00, 0x06, b'h', b'e', b'l', b'l', b'o',
    ]
  );
}

#[test]
fn decodes_inline_lzvn_wrapper() {
  let encoded = encode_decmpfs(b"hello").expect("inline decmpfs encode should succeed");

  let decoded = decode_decmpfs(&encoded.xattr, None).expect("inline decmpfs decode should succeed");
  assert_eq!(decoded, b"hello");
}

#[test]
fn decode_into_uses_existing_output_buffer() {
  let encoded = encode_decmpfs(b"hello").expect("inline decmpfs encode should succeed");
  let mut dst = [0_u8; 5];

  let written = decode_decmpfs_into(&encoded.xattr, None, &mut dst)
    .expect("decode_into should write into caller buffer");
  assert_eq!(written, 5);
  assert_eq!(&dst, b"hello");
}

#[test]
fn keeps_highly_compressible_large_input_inline() {
  let source = vec![b'a'; 10_000];

  let encoded = encode_decmpfs(&source).expect("compressible input should encode");
  let header = parse_decmpfs_header(&encoded.xattr).expect("header should parse");

  assert_eq!(header.compression_type, DecmpfsCompressionType::LzvnXattr);
  assert!(encoded.resource_fork.is_none());
  assert_eq!(
    decode_decmpfs(&encoded.xattr, None).expect("decode should succeed"),
    source
  );
}

#[test]
fn uses_resource_fork_for_large_incompressible_input() {
  let source = pseudo_random_bytes(10_000);

  let encoded = encode_decmpfs(&source).expect("resource-fork encode should succeed");
  let header = parse_decmpfs_header(&encoded.xattr).expect("header should parse");

  assert_eq!(
    header.compression_type,
    DecmpfsCompressionType::LzvnResourceFork
  );
  let resource_fork = encoded
    .resource_fork
    .as_deref()
    .expect("resource fork should be present");
  assert_eq!(
    decode_decmpfs(&encoded.xattr, Some(resource_fork)).expect("decode should succeed"),
    source
  );
}

#[test]
fn resource_fork_roundtrips_multiple_blocks() {
  let source = pseudo_random_bytes(70_000);
  let encoded = encode_decmpfs(&source).expect("multi-block encode should succeed");
  let resource_fork = encoded
    .resource_fork
    .as_deref()
    .expect("resource fork should be present");

  let decoded =
    decode_decmpfs(&encoded.xattr, Some(resource_fork)).expect("multi-block decode should succeed");
  assert_eq!(decoded, source);
}

#[test]
fn rejects_missing_resource_fork() {
  let xattr = [
    b'f', b'p', b'm', b'c', 0x08, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
  ];

  let err =
    decode_decmpfs(&xattr, None).expect_err("resource-fork type should require resource fork");
  assert_eq!(err, Error::MissingResourceFork);
}

#[test]
fn rejects_unsupported_compression_type() {
  let xattr = [
    b'f', b'p', b'm', b'c', 0x0B, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
  ];

  let err = decode_decmpfs(&xattr, None).expect_err("unsupported compression type should fail");
  assert_eq!(
    err,
    Error::UnsupportedDecmpfsCompressionType {
      compression_type: 11,
    }
  );
}

#[test]
fn rejects_invalid_resource_fork_table() {
  let source = pseudo_random_bytes(10_000);
  let encoded = encode_decmpfs(&source).expect("resource-fork encode should succeed");
  let mut resource_fork = encoded.resource_fork.expect("resource fork should exist");
  resource_fork[0] = 0;

  let err = decode_decmpfs(&encoded.xattr, Some(&resource_fork))
    .expect_err("invalid resource fork should fail");
  assert_eq!(
    err,
    Error::InvalidResourceFork {
      reason: "first block offset does not match offset table size",
    }
  );
}

fn pseudo_random_bytes(len: usize) -> Vec<u8> {
  let mut state = 0x1234_5678_u32;
  let mut bytes = Vec::with_capacity(len);
  for _ in 0..len {
    state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    bytes.push((state >> 24) as u8);
  }
  bytes
}

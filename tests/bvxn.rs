//! Integration tests for Apple `bvxn` decoding.

use lzvn::{BVXN_MAGIC, Error, decode_bvxn, decode_bvxn_into, parse_bvxn_header};

const EOS: [u8; 8] = [0x06, 0, 0, 0, 0, 0, 0, 0];

#[test]
fn parses_bvxn_header() {
  let block = bvxn_block(raw_hello_payload(), 5);

  let header = parse_bvxn_header(&block).expect("header should parse");
  assert_eq!(header.raw_bytes, 5);
  assert_eq!(header.payload_bytes, 14);
}

#[test]
fn decodes_bvxn_block() {
  let block = bvxn_block(raw_hello_payload(), 5);

  let decoded = decode_bvxn(&block).expect("bvxn block should decode");
  assert_eq!(decoded, b"hello");
}

#[test]
fn decodes_bvxn_block_into_existing_buffer() {
  let block = bvxn_block(raw_hello_payload(), 5);
  let mut decoded = [0_u8; 5];

  let written = decode_bvxn_into(&block, &mut decoded).expect("decode_into should work");
  assert_eq!(written, 5);
  assert_eq!(&decoded, b"hello");
}

#[test]
fn rejects_wrong_magic() {
  let mut block = bvxn_block(raw_hello_payload(), 5);
  block[0..4].copy_from_slice(&0x3178_7662_u32.to_le_bytes());

  let err = decode_bvxn(&block).expect_err("wrong magic should fail");
  assert_eq!(err, Error::UnsupportedBlockMagic { magic: 0x3178_7662 });
}

#[test]
fn rejects_truncated_block() {
  let mut block = bvxn_block(raw_hello_payload(), 5);
  block.pop();

  let err = decode_bvxn(&block).expect_err("truncated block should fail");
  assert_eq!(
    err,
    Error::TruncatedInput {
      position: block.len()
    }
  );
}

#[test]
fn rejects_size_mismatch_between_header_and_payload() {
  let block = bvxn_block(raw_hello_payload(), 6);

  let err = decode_bvxn(&block).expect_err("header/raw size mismatch should fail");
  assert_eq!(
    err,
    Error::SizeMismatch {
      expected: 6,
      actual: 5,
    }
  );
}

#[test]
fn exposes_public_magic_constant() {
  assert_eq!(BVXN_MAGIC, 0x6E78_7662);
}

fn raw_hello_payload() -> Vec<u8> {
  let mut payload = vec![0xE5, b'h', b'e', b'l', b'l', b'o'];
  payload.extend_from_slice(&EOS);
  payload
}

fn bvxn_block(payload: Vec<u8>, raw_bytes: u32) -> Vec<u8> {
  let mut block = Vec::new();
  block.extend_from_slice(&BVXN_MAGIC.to_le_bytes());
  block.extend_from_slice(&raw_bytes.to_le_bytes());
  block.extend_from_slice(&(payload.len() as u32).to_le_bytes());
  block.extend_from_slice(&payload);
  block
}

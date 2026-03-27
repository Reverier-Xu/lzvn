//! Integration tests for raw LZVN and Apple `bvxn` encoding.

use lzvn::{BVXN_MAGIC, decode_bvxn, decode_raw, encode_bvxn, encode_raw};

const EOS: [u8; 8] = [0x06, 0, 0, 0, 0, 0, 0, 0];

#[test]
fn encodes_empty_input_as_end_marker_only() {
  assert_eq!(encode_raw(b""), EOS);
}

#[test]
fn encodes_small_literal_stream_exactly() {
  let encoded = encode_raw(b"hello");

  assert_eq!(
    encoded,
    [
      0xe5, b'h', b'e', b'l', b'l', b'o', 0x06, 0, 0, 0, 0, 0, 0, 0
    ]
  );
}

#[test]
fn encodes_overlap_match_exactly() {
  let encoded = encode_raw(b"aaaaa");

  assert_eq!(encoded, [0x48, 0x01, b'a', 0x06, 0, 0, 0, 0, 0, 0, 0]);
}

#[test]
fn encodes_medium_distance_match_exactly() {
  let encoded = encode_raw(b"abcabcabc");

  assert_eq!(
    encoded,
    [
      0xb8, 0x0f, 0x00, b'a', b'b', b'c', 0x06, 0, 0, 0, 0, 0, 0, 0
    ]
  );
}

#[test]
fn raw_encoder_roundtrips_repetitive_data() {
  let source = b"bananabananabananabananabanana";

  let encoded = encode_raw(source);
  let decoded = decode_raw(&encoded, source.len()).expect("roundtrip decode should work");

  assert_eq!(decoded, source);
  assert!(encoded.len() < source.len() + EOS.len());
}

#[test]
fn bvxn_encoder_roundtrips() {
  let block = encode_bvxn(b"hello").expect("bvxn encode should succeed");

  assert_eq!(&block[0..4], &BVXN_MAGIC.to_le_bytes());
  assert_eq!(
    decode_bvxn(&block).expect("bvxn decode should succeed"),
    b"hello"
  );
}

#[test]
fn bvxn_encoder_produces_expected_small_block() {
  let block = encode_bvxn(b"hello").expect("small bvxn encode should succeed");

  assert_eq!(
    block,
    [
      b'b', b'v', b'x', b'n', 0x05, 0x00, 0x00, 0x00, 0x0e, 0x00, 0x00, 0x00, 0xe5, b'h', b'e',
      b'l', b'l', b'o', 0x06, 0, 0, 0, 0, 0, 0, 0,
    ]
  );
}

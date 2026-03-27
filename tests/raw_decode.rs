//! Integration tests for raw LZVN decoding.

use lzvn::{Error, decode_raw, decode_raw_into};

const EOS: [u8; 8] = [0x06, 0, 0, 0, 0, 0, 0, 0];

#[test]
fn decodes_small_literal() {
  let encoded = [
    0xE5, b'h', b'e', b'l', b'l', b'o', 0x06, 0, 0, 0, 0, 0, 0, 0,
  ];

  let decoded = decode_raw(&encoded, 5).expect("small literal should decode");
  assert_eq!(decoded, b"hello");
}

#[test]
fn decodes_small_literal_with_compact_eos() {
  let encoded = [0xE5, b'h', b'e', b'l', b'l', b'o', 0x06];

  let decoded = decode_raw(&encoded, 5).expect("compact eos should decode");
  assert_eq!(decoded, b"hello");
}

#[test]
fn decodes_large_literal() {
  let literal = b"abcdefghijklmnopqrst";
  let mut encoded = vec![0xE0, (literal.len() - 16) as u8];
  encoded.extend_from_slice(literal);
  encoded.extend_from_slice(&EOS);

  let decoded = decode_raw(&encoded, literal.len()).expect("large literal should decode");
  assert_eq!(decoded, literal);
}

#[test]
fn decodes_small_distance_match() {
  let encoded = [0xC0, 0x03, b'a', b'b', b'c', 0x06, 0, 0, 0, 0, 0, 0, 0];

  let decoded = decode_raw(&encoded, 6).expect("small-distance match should decode");
  assert_eq!(decoded, b"abcabc");
}

#[test]
fn decodes_previous_distance_and_small_match() {
  let encoded = [
    0x40, 0x01, b'a', 0x46, b'b', 0xF4, 0x06, 0, 0, 0, 0, 0, 0, 0,
  ];

  let decoded = decode_raw(&encoded, 12).expect("previous-distance path should decode");
  assert_eq!(decoded, b"aaaabbbbbbbb");
}

#[test]
fn decodes_large_distance_opcode() {
  let encoded = [0x47, 0x01, 0x00, b'a', 0x06, 0, 0, 0, 0, 0, 0, 0];

  let decoded = decode_raw(&encoded, 4).expect("large-distance opcode should decode");
  assert_eq!(decoded, b"aaaa");
}

#[test]
fn decodes_large_match() {
  let encoded = [0x40, 0x01, b'a', 0xF0, 0x04, 0x06, 0, 0, 0, 0, 0, 0, 0];

  let decoded = decode_raw(&encoded, 24).expect("large match should decode");
  assert_eq!(decoded, vec![b'a'; 24]);
}

#[test]
fn decodes_medium_distance_match() {
  let literal = vec![b'x'; 1_536];
  let mut encoded = literal_only_stream(&literal);
  encoded.extend_from_slice(&[0xA0, 0x00, 0x18]);
  encoded.extend_from_slice(&EOS);

  let decoded = decode_raw(&encoded, 1_539).expect("medium-distance opcode should decode");
  assert_eq!(&decoded[..1_536], literal.as_slice());
  assert_eq!(&decoded[1_536..], b"xxx");
}

#[test]
fn decodes_overlap_match() {
  let encoded = [0x48, 0x01, b'a', 0x06, 0, 0, 0, 0, 0, 0, 0];

  let decoded = decode_raw(&encoded, 5).expect("overlap match should decode");
  assert_eq!(decoded, b"aaaaa");
}

#[test]
fn rejects_invalid_opcode() {
  let mut encoded = vec![0x1E];
  encoded.extend_from_slice(&EOS);

  let err = decode_raw(&encoded, 0).expect_err("invalid opcode should fail");
  assert_eq!(
    err,
    Error::InvalidOpcode {
      position: 0,
      opcode: 0x1E,
    }
  );
}

#[test]
fn rejects_invalid_match_distance() {
  let encoded = [0x40, 0x00, b'a', 0x06, 0, 0, 0, 0, 0, 0, 0];

  let err = decode_raw(&encoded, 4).expect_err("distance zero should fail");
  assert_eq!(
    err,
    Error::InvalidMatchDistance {
      distance: 0,
      available: 1,
    }
  );
}

#[test]
fn rejects_truncated_input() {
  let encoded = [0xE1, b'a'];

  let err = decode_raw(&encoded, 1).expect_err("missing end marker should fail");
  assert_eq!(err, Error::TruncatedInput { position: 2 });
}

#[test]
fn rejects_trailing_bytes() {
  let mut encoded = vec![0xE1, b'a'];
  encoded.extend_from_slice(&EOS);
  encoded.push(0x00);

  let err = decode_raw(&encoded, 1).expect_err("trailing data should fail");
  assert_eq!(err, Error::TrailingData { remaining: 1 });
}

#[test]
fn decode_into_reports_output_too_small() {
  let encoded = [
    0xE5, b'h', b'e', b'l', b'l', b'o', 0x06, 0, 0, 0, 0, 0, 0, 0,
  ];
  let mut decoded = [0_u8; 4];

  let err = decode_raw_into(&encoded, &mut decoded).expect_err("small output should fail");
  assert_eq!(
    err,
    Error::OutputTooSmall {
      written: 0,
      capacity: 4,
    }
  );
}

#[test]
fn reports_size_mismatch_for_larger_than_actual_output() {
  let encoded = [
    0xE5, b'h', b'e', b'l', b'l', b'o', 0x06, 0, 0, 0, 0, 0, 0, 0,
  ];

  let err = decode_raw(&encoded, 6).expect_err("length mismatch should fail");
  assert_eq!(
    err,
    Error::SizeMismatch {
      expected: 6,
      actual: 5,
    }
  );
}

fn literal_only_stream(bytes: &[u8]) -> Vec<u8> {
  let mut encoded = Vec::new();
  let mut offset = 0;

  while offset < bytes.len() {
    let remaining = bytes.len() - offset;
    if remaining > 15 {
      let chunk_len = remaining.min(271);
      encoded.push(0xE0);
      encoded.push((chunk_len - 16) as u8);
      encoded.extend_from_slice(&bytes[offset..offset + chunk_len]);
      offset += chunk_len;
    } else {
      encoded.push(0xE0 + remaining as u8);
      encoded.extend_from_slice(&bytes[offset..]);
      offset = bytes.len();
    }
  }

  encoded
}

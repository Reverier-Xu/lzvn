//! Differential tests against the Apple-compatible `lzfse` reference decoder.

use lzfse::decode_buffer;
use lzvn::{
  DECMPFS_CHUNK_SIZE, DecmpfsCompressionType, decode_bvxn, decode_decmpfs, decode_raw, encode_bvxn,
  encode_decmpfs, parse_decmpfs_header,
};

const BLACKTOP_LZVN_ENC: &[u8] = include_bytes!("fixtures/bin/blacktop_lzvn_enc.bin");
const BLACKTOP_LZVN_DEC: &[u8] = include_bytes!("fixtures/bin/blacktop_lzvn_dec.bin");
const LIBFSAPFS_LZVN_COMPRESSED: &[u8] =
  include_bytes!("fixtures/bin/libfsapfs_lzvn_compressed.bin");
const LIBFSAPFS_LZVN_EXPECTED: &[u8] = include_bytes!("fixtures/bin/libfsapfs_lzvn_expected.bin");
const LIBFSAPFS_DECMPFS_LZVN_XATTR: &[u8] =
  include_bytes!("fixtures/bin/libfsapfs_decmpfs_lzvn_xattr.bin");

#[test]
fn reference_decoder_matches_real_wrapped_raw_fixtures() {
  for (raw, expected) in [
    (BLACKTOP_LZVN_ENC, BLACKTOP_LZVN_DEC),
    (LIBFSAPFS_LZVN_COMPRESSED, LIBFSAPFS_LZVN_EXPECTED),
  ] {
    let wrapped = wrap_raw_as_bvxn(raw, expected.len());
    let ours = decode_raw(raw, expected.len()).expect("our raw decoder should accept fixture");
    let reference = decode_with_reference(&wrapped, expected.len());
    assert_eq!(ours, expected);
    assert_eq!(reference, expected);
  }
}

#[test]
fn reference_decoder_accepts_our_bvxn_output() {
  for sample in sample_inputs() {
    let encoded = encode_bvxn(sample).expect("bvxn encode should succeed");
    let ours = decode_bvxn(&encoded).expect("our bvxn decode should succeed");
    let reference = decode_with_reference(&with_lzfse_eos(&encoded), sample.len());
    assert_eq!(ours, sample);
    assert_eq!(reference, sample);
  }
}

#[test]
fn reference_decoder_accepts_our_decmpfs_blocks() {
  for sample in sample_inputs() {
    let encoded = encode_decmpfs(sample).expect("decmpfs encode should succeed");
    let ours = decode_decmpfs(&encoded.xattr, encoded.resource_fork.as_deref())
      .expect("our decmpfs decode should succeed");
    assert_eq!(ours, sample);

    let header = parse_decmpfs_header(&encoded.xattr).expect("decmpfs header should parse");
    match header.compression_type {
      DecmpfsCompressionType::LzvnXattr => {
        let payload = &encoded.xattr[16..];
        let reference = decode_lzvn_wrapper_block(payload, sample.len());
        assert_eq!(reference, sample);
      }
      DecmpfsCompressionType::LzvnResourceFork => {
        let resource_fork = encoded
          .resource_fork
          .as_deref()
          .expect("resource fork should exist");
        let mut rebuilt = Vec::with_capacity(sample.len());
        let block_count = sample.len().div_ceil(DECMPFS_CHUNK_SIZE);
        for block_index in 0..block_count {
          let start = read_offset(resource_fork, block_index);
          let end = read_offset(resource_fork, block_index + 1);
          let expected_len = block_len(sample.len(), block_index);
          rebuilt.extend_from_slice(&decode_lzvn_wrapper_block(
            &resource_fork[start..end],
            expected_len,
          ));
        }
        assert_eq!(rebuilt, sample);
      }
      other => panic!("unexpected decmpfs compression type: {other:?}"),
    }
  }
}

#[test]
fn reference_decoder_matches_real_decmpfs_fixture() {
  let header =
    parse_decmpfs_header(LIBFSAPFS_DECMPFS_LZVN_XATTR).expect("fixture header should parse");
  assert_eq!(header.compression_type, DecmpfsCompressionType::LzvnXattr);
  let payload = &LIBFSAPFS_DECMPFS_LZVN_XATTR[16..];

  let ours = decode_decmpfs(LIBFSAPFS_DECMPFS_LZVN_XATTR, None).expect("fixture should decode");
  let reference = decode_lzvn_wrapper_block(payload, header.uncompressed_size);
  assert_eq!(ours, LIBFSAPFS_LZVN_EXPECTED);
  assert_eq!(reference, LIBFSAPFS_LZVN_EXPECTED);
}

fn decode_with_reference(encoded: &[u8], expected_len: usize) -> Vec<u8> {
  let mut output = vec![0; expected_len + 64];
  let written =
    decode_buffer(encoded, &mut output).expect("reference decoder should accept fixture");
  assert_eq!(written, expected_len);
  output.truncate(written);
  output
}

fn decode_lzvn_wrapper_block(block: &[u8], expected_len: usize) -> Vec<u8> {
  if block.len() == expected_len + 1 && block.first() == Some(&0x06) {
    return block[1..].to_vec();
  }

  let wrapped = wrap_raw_as_bvxn(block, expected_len);
  decode_with_reference(&wrapped, expected_len)
}

fn wrap_raw_as_bvxn(raw: &[u8], decoded_len: usize) -> Vec<u8> {
  let canonical = canonicalize_raw_for_reference(raw);
  let mut wrapped = Vec::with_capacity(16 + canonical.len());
  wrapped.extend_from_slice(b"bvxn");
  wrapped.extend_from_slice(&(decoded_len as u32).to_le_bytes());
  wrapped.extend_from_slice(&(canonical.len() as u32).to_le_bytes());
  wrapped.extend_from_slice(&canonical);
  with_lzfse_eos(&wrapped)
}

fn canonicalize_raw_for_reference(raw: &[u8]) -> Vec<u8> {
  if raw.last() == Some(&0x06) && !raw.ends_with(&[0x06, 0, 0, 0, 0, 0, 0, 0]) {
    let mut canonical = Vec::with_capacity(raw.len() + 7);
    canonical.extend_from_slice(raw);
    canonical.extend_from_slice(&[0; 7]);
    canonical
  } else {
    raw.to_vec()
  }
}

fn with_lzfse_eos(block: &[u8]) -> Vec<u8> {
  let mut encoded = Vec::with_capacity(block.len() + 4);
  encoded.extend_from_slice(block);
  encoded.extend_from_slice(b"bvx$");
  encoded
}

fn read_offset(resource_fork: &[u8], index: usize) -> usize {
  let start = index * 4;
  let end = start + 4;
  u32::from_le_bytes(
    resource_fork[start..end]
      .try_into()
      .expect("offset slice has exact length"),
  ) as usize
}

fn block_len(total_len: usize, block_index: usize) -> usize {
  let start = block_index * DECMPFS_CHUNK_SIZE;
  (total_len - start).min(DECMPFS_CHUNK_SIZE)
}

fn sample_inputs() -> Vec<&'static [u8]> {
  static EMPTY: [u8; 0] = [];
  static HELLO: &[u8] = b"hello";
  static BANANA: &[u8] = b"bananabananabananabanana";
  static TEXT: &[u8] =
    b"lorem ipsum dolor sit amet, consectetur adipiscing elit. lorem ipsum dolor sit amet.";
  static REPEAT: [u8; 4096] = [b'a'; 4096];
  static LARGE: [u8; 70000] = [b'z'; 70000];

  vec![&EMPTY, HELLO, BANANA, TEXT, &REPEAT, &LARGE]
}

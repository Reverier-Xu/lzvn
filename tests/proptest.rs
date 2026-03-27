//! Property-based tests for raw, `bvxn`, and `decmpfs` support.

use lzvn::{
  BvxnDecoder, BvxnEncoder, RawDecoder, RawEncoder, StreamStatus, decode_bvxn, decode_decmpfs,
  decode_raw, encode_bvxn, encode_decmpfs, encode_raw,
};
use proptest::collection::vec;
use proptest::prelude::*;

fn byte_data(max_len: usize) -> impl Strategy<Value = Vec<u8>> {
  vec(any::<u8>(), 0..max_len)
}

fn decmpfs_data() -> impl Strategy<Value = Vec<u8>> {
  prop_oneof![
    vec(any::<u8>(), 0..4_096),
    vec(any::<u8>(), 65_520..65_600),
    vec(any::<u8>(), 70_000..70_080),
  ]
}

fn drain_raw_encoder(input: &[u8], input_chunk: usize, output_chunk: usize) -> Vec<u8> {
  let mut encoder = RawEncoder::new();
  for chunk in input.chunks(input_chunk) {
    encoder.feed(chunk);
  }
  encoder.finish_input();

  let mut encoded = Vec::new();
  loop {
    let mut chunk = vec![0_u8; output_chunk];
    let progress = encoder
      .encode_into(&mut chunk)
      .expect("incremental raw encode should succeed");
    encoded.extend_from_slice(&chunk[..progress.written]);
    if progress.status == StreamStatus::Finished {
      return encoded;
    }
  }
}

fn drain_raw_decoder(input: &[u8], input_chunk: usize, output_chunk: usize) -> Vec<u8> {
  let mut decoder = RawDecoder::new();
  let mut decoded = Vec::new();
  let mut offset = 0;
  let mut finished_input = false;

  loop {
    if offset < input.len() {
      let end = (offset + input_chunk).min(input.len());
      decoder.feed(&input[offset..end]);
      offset = end;
    } else if !finished_input {
      decoder.finish_input();
      finished_input = true;
    }

    loop {
      let mut chunk = vec![0_u8; output_chunk];
      let progress = decoder
        .decode_into(&mut chunk)
        .expect("incremental raw decode should succeed");
      decoded.extend_from_slice(&chunk[..progress.written]);
      match progress.status {
        StreamStatus::NeedOutput => continue,
        StreamStatus::NeedInput => break,
        StreamStatus::Finished => return decoded,
      }
    }
  }
}

fn drain_bvxn_encoder(input: &[u8], input_chunk: usize, output_chunk: usize) -> Vec<u8> {
  let mut encoder = BvxnEncoder::new();
  for chunk in input.chunks(input_chunk) {
    encoder.feed(chunk);
  }
  encoder.finish_input();

  let mut encoded = Vec::new();
  loop {
    let mut chunk = vec![0_u8; output_chunk];
    let progress = encoder
      .encode_into(&mut chunk)
      .expect("incremental bvxn encode should succeed");
    encoded.extend_from_slice(&chunk[..progress.written]);
    if progress.status == StreamStatus::Finished {
      return encoded;
    }
  }
}

fn drain_bvxn_decoder(input: &[u8], input_chunk: usize, output_chunk: usize) -> Vec<u8> {
  let mut decoder = BvxnDecoder::new();
  let mut decoded = Vec::new();
  let mut offset = 0;
  let mut finished_input = false;

  loop {
    if offset < input.len() {
      let end = (offset + input_chunk).min(input.len());
      decoder.feed(&input[offset..end]);
      offset = end;
    } else if !finished_input {
      decoder.finish_input();
      finished_input = true;
    }

    loop {
      let mut chunk = vec![0_u8; output_chunk];
      let progress = decoder
        .decode_into(&mut chunk)
        .expect("incremental bvxn decode should succeed");
      decoded.extend_from_slice(&chunk[..progress.written]);
      match progress.status {
        StreamStatus::NeedOutput => continue,
        StreamStatus::NeedInput => break,
        StreamStatus::Finished => return decoded,
      }
    }
  }
}

proptest! {
  #![proptest_config(ProptestConfig {
    cases: 64,
    .. ProptestConfig::default()
  })]

  #[test]
  fn raw_roundtrip(data in byte_data(4_096)) {
    let encoded = encode_raw(&data);
    let decoded = decode_raw(&encoded, data.len()).expect("raw roundtrip should decode");
    prop_assert_eq!(decoded, data);
  }

  #[test]
  fn raw_stream_matches_one_shot(
    data in byte_data(2_048),
    input_chunk in 1_usize..64,
    output_chunk in 1_usize..64,
  ) {
    let expected_encoded = encode_raw(&data);
    let streamed_encoded = drain_raw_encoder(&data, input_chunk, output_chunk);
    prop_assert_eq!(streamed_encoded.as_slice(), expected_encoded.as_slice());

    let streamed_decoded = drain_raw_decoder(&expected_encoded, input_chunk, output_chunk);
    prop_assert_eq!(streamed_decoded, data);
  }

  #[test]
  fn bvxn_roundtrip(data in byte_data(4_096)) {
    let encoded = encode_bvxn(&data).expect("bvxn encode should succeed");
    let decoded = decode_bvxn(&encoded).expect("bvxn decode should succeed");
    prop_assert_eq!(decoded, data);
  }

  #[test]
  fn bvxn_stream_matches_one_shot(
    data in byte_data(2_048),
    input_chunk in 1_usize..64,
    output_chunk in 1_usize..64,
  ) {
    let expected_encoded = encode_bvxn(&data).expect("bvxn encode should succeed");
    let streamed_encoded = drain_bvxn_encoder(&data, input_chunk, output_chunk);
    prop_assert_eq!(streamed_encoded.as_slice(), expected_encoded.as_slice());

    let streamed_decoded = drain_bvxn_decoder(&expected_encoded, input_chunk, output_chunk);
    prop_assert_eq!(streamed_decoded, data);
  }
}

proptest! {
  #![proptest_config(ProptestConfig {
    cases: 16,
    .. ProptestConfig::default()
  })]

  #[test]
  fn decmpfs_roundtrip(data in decmpfs_data()) {
    let encoded = encode_decmpfs(&data).expect("decmpfs encode should succeed");
    let decoded = decode_decmpfs(&encoded.xattr, encoded.resource_fork.as_deref())
      .expect("decmpfs decode should succeed");
    prop_assert_eq!(decoded, data);
  }
}

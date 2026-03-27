//! Integration tests for incremental encoder and decoder state machines.

use lzvn::{
  BvxnDecoder, BvxnEncoder, RawDecoder, RawEncoder, StreamStatus, decode_bvxn, decode_raw,
  encode_bvxn, encode_raw,
};

#[test]
fn raw_decoder_handles_chunked_input_and_output() {
  let source = b"bananabananabananabanana";
  let encoded = encode_raw(source);
  let mut decoder = RawDecoder::new();
  let mut decoded = Vec::new();
  let mut offset = 0;

  loop {
    if offset < encoded.len() {
      let end = (offset + 2).min(encoded.len());
      decoder.feed(&encoded[offset..end]);
      offset = end;
    } else {
      decoder.finish_input();
    }

    loop {
      let mut chunk = [0_u8; 3];
      let progress = decoder
        .decode_into(&mut chunk)
        .expect("incremental raw decode should succeed");
      decoded.extend_from_slice(&chunk[..progress.written]);

      match progress.status {
        StreamStatus::NeedOutput => continue,
        StreamStatus::NeedInput => break,
        StreamStatus::Finished => {
          assert_eq!(decoded, source);
          assert!(decoder.is_finished());
          assert_eq!(decoder.total_output(), source.len());
          return;
        }
      }
    }
  }
}

#[test]
fn raw_encoder_buffers_until_finished_input() {
  let mut encoder = RawEncoder::new();
  encoder.feed(b"banana");

  let mut scratch = [0_u8; 8];
  let progress = encoder
    .encode_into(&mut scratch)
    .expect("encoder should request more input first");
  assert_eq!(progress.written, 0);
  assert_eq!(progress.status, StreamStatus::NeedInput);

  encoder.feed(b"banana");
  encoder.finish_input();

  let mut encoded = Vec::new();
  loop {
    let mut chunk = [0_u8; 4];
    let progress = encoder
      .encode_into(&mut chunk)
      .expect("incremental raw encode should succeed");
    encoded.extend_from_slice(&chunk[..progress.written]);
    if progress.status == StreamStatus::Finished {
      break;
    }
  }

  assert!(encoder.is_finished());
  assert_eq!(
    decode_raw(&encoded, 12).expect("roundtrip should decode"),
    b"bananabanana"
  );
}

#[test]
fn bvxn_decoder_handles_chunked_block() {
  let source = b"hello hello hello";
  let block = encode_bvxn(source).expect("bvxn encode should succeed");
  let mut decoder = BvxnDecoder::new();
  let mut decoded = Vec::new();
  let mut offset = 0;

  loop {
    if offset < block.len() {
      let end = (offset + 3).min(block.len());
      decoder.feed(&block[offset..end]);
      offset = end;
    } else {
      decoder.finish_input();
    }

    loop {
      let mut chunk = [0_u8; 5];
      let progress = decoder
        .decode_into(&mut chunk)
        .expect("incremental bvxn decode should succeed");
      decoded.extend_from_slice(&chunk[..progress.written]);

      match progress.status {
        StreamStatus::NeedOutput => continue,
        StreamStatus::NeedInput => break,
        StreamStatus::Finished => {
          assert_eq!(decoded, source);
          assert!(decoder.is_finished());
          return;
        }
      }
    }
  }
}

#[test]
fn bvxn_encoder_buffers_until_finished_input() {
  let mut encoder = BvxnEncoder::new();
  encoder.feed(b"hello ");

  let mut scratch = [0_u8; 8];
  let progress = encoder
    .encode_into(&mut scratch)
    .expect("encoder should request more input first");
  assert_eq!(progress.written, 0);
  assert_eq!(progress.status, StreamStatus::NeedInput);

  encoder.feed(b"world");
  encoder.finish_input();

  let mut block = Vec::new();
  loop {
    let mut chunk = [0_u8; 6];
    let progress = encoder
      .encode_into(&mut chunk)
      .expect("incremental bvxn encode should succeed");
    block.extend_from_slice(&chunk[..progress.written]);
    if progress.status == StreamStatus::Finished {
      break;
    }
  }

  assert!(encoder.is_finished());
  assert_eq!(
    decode_bvxn(&block).expect("encoded block should decode"),
    b"hello world"
  );
}

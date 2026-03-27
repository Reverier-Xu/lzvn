#![no_main]

use libfuzzer_sys::fuzz_target;

const MAX_DECODED_LEN: usize = 1 << 20;

fuzz_target!(|data: &[u8]| {
  if data.len() < 4 {
    return;
  }

  let expected_len =
    u32::from_le_bytes(data[0..4].try_into().expect("slice has exact length")) as usize;
  if expected_len > MAX_DECODED_LEN {
    return;
  }

  let encoded = &data[4..];
  let decoded = lzvn::decode_raw(encoded, expected_len);
  let mut output = vec![0; expected_len];
  let decoded_into = lzvn::decode_raw_into(encoded, &mut output);

  match (decoded, decoded_into) {
    (Ok(decoded), Ok(written)) => {
      assert_eq!(written, decoded.len());
      assert_eq!(&output[..written], decoded.as_slice());
    }
    (Err(_), Err(_)) => {}
    (left, right) => panic!("raw decode mismatch: {left:?} vs {right:?}"),
  }
});

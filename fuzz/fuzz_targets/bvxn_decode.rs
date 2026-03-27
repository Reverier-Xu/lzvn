#![no_main]

use libfuzzer_sys::fuzz_target;

const MAX_DECODED_LEN: usize = 1 << 20;

fuzz_target!(|data: &[u8]| {
  let parsed = lzvn::parse_bvxn_header(data);
  let decoded = lzvn::decode_bvxn(data);

  match parsed {
    Ok(header) if header.raw_bytes <= MAX_DECODED_LEN => {
      let mut output = vec![0; header.raw_bytes];
      let decoded_into = lzvn::decode_bvxn_into(data, &mut output);

      match (decoded, decoded_into) {
        (Ok(decoded), Ok(written)) => {
          assert_eq!(written, decoded.len());
          assert_eq!(&output[..written], decoded.as_slice());
        }
        (Err(_), Err(_)) => {}
        (left, right) => panic!("bvxn decode mismatch: {left:?} vs {right:?}"),
      }
    }
    _ => {
      let _ = decoded;
    }
  }
});

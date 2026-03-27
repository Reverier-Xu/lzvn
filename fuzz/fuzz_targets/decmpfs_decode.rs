#![no_main]

use libfuzzer_sys::fuzz_target;

const MAX_DECODED_LEN: usize = 1 << 20;

fuzz_target!(|data: &[u8]| {
  if data.len() < 4 {
    return;
  }

  let xattr_len =
    u32::from_le_bytes(data[0..4].try_into().expect("slice has exact length")) as usize;
  if xattr_len > data.len() - 4 {
    return;
  }

  let xattr = &data[4..4 + xattr_len];
  let resource_fork = &data[4 + xattr_len..];
  let parsed = lzvn::parse_decmpfs_header(xattr);
  let decoded = lzvn::decode_decmpfs(xattr, Some(resource_fork));

  match parsed {
    Ok(header) if header.uncompressed_size <= MAX_DECODED_LEN => {
      let mut output = vec![0; header.uncompressed_size];
      let decoded_into = lzvn::decode_decmpfs_into(xattr, Some(resource_fork), &mut output);

      match (decoded, decoded_into) {
        (Ok(decoded), Ok(written)) => {
          assert_eq!(written, decoded.len());
          assert_eq!(&output[..written], decoded.as_slice());
        }
        (Err(_), Err(_)) => {}
        (left, right) => panic!("decmpfs decode mismatch: {left:?} vs {right:?}"),
      }
    }
    _ => {
      let _ = decoded;
    }
  }
});

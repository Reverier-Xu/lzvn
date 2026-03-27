#![no_main]

use libfuzzer_sys::fuzz_target;

const MAX_INPUT_LEN: usize = 1 << 18;

fuzz_target!(|data: &[u8]| {
  if data.len() > MAX_INPUT_LEN {
    return;
  }

  let raw = lzvn::encode_raw(data);
  let raw_decoded = lzvn::decode_raw(&raw, data.len()).expect("raw roundtrip should decode");
  assert_eq!(raw_decoded, data);

  let bvxn = lzvn::encode_bvxn(data).expect("bvxn encode should succeed");
  let bvxn_decoded = lzvn::decode_bvxn(&bvxn).expect("bvxn roundtrip should decode");
  assert_eq!(bvxn_decoded, data);

  let decmpfs = lzvn::encode_decmpfs(data).expect("decmpfs encode should succeed");
  let decmpfs_decoded = lzvn::decode_decmpfs(&decmpfs.xattr, decmpfs.resource_fork.as_deref())
    .expect("decmpfs roundtrip should decode");
  assert_eq!(decmpfs_decoded, data);
});

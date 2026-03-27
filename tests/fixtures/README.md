# Real Fixture Sources

These fixture files are vendored from public internet sources to provide
end-to-end regression coverage against independently produced LZVN-related
data.

Layout:

- `bin/` contains binary fixture images used by the e2e tests
- `*.hex` contains ASCII hex companions for inspection and review

Sources:

- `bin/blacktop_lzvn_enc.bin`
  - Source: `https://raw.githubusercontent.com/blacktop/lzfse-cgo/9aa17847f933b151210446ffd54325925d125f65/test/lzvn_enc.bin`
  - License: MIT
  - Meaning: original raw LZVN binary fixture for the plaintext in `bin/blacktop_lzvn_dec.bin`
- `bin/blacktop_lzvn_dec.bin`
  - Source: `https://raw.githubusercontent.com/blacktop/lzfse-cgo/9aa17847f933b151210446ffd54325925d125f65/test/lzvn_dec.bin`
  - License: MIT
  - Meaning: original decoded plaintext fixture for `bin/blacktop_lzvn_enc.bin`
- `bin/libfsapfs_lzvn_compressed.bin`
  - Source: `https://raw.githubusercontent.com/libyal/libfsapfs/f179325e5405d3b09a314348646e9898b722759f/tests/fsapfs_test_compression.c`
  - License: LGPL-3.0-or-later
  - Meaning: binary image of the small Apple-oriented LZVN compressed block used by libfsapfs tests
- `bin/libfsapfs_lzvn_expected.bin`
  - Source: `https://raw.githubusercontent.com/libyal/libfsapfs/f179325e5405d3b09a314348646e9898b722759f/tests/fsapfs_test_compression.c`
  - License: LGPL-3.0-or-later
  - Meaning: expected decoded bytes for `bin/libfsapfs_lzvn_compressed.bin`
- `bin/libfsapfs_lzvn_uncompressed_sentinel.bin`
  - Source: `https://raw.githubusercontent.com/libyal/libfsapfs/f179325e5405d3b09a314348646e9898b722759f/tests/fsapfs_test_compression.c`
  - License: LGPL-3.0-or-later
  - Meaning: original binary image of an Apple LZVN wrapper payload that uses the `0x06` uncompressed sentinel
- `bin/libfsapfs_decmpfs_lzvn_xattr.bin`
  - Source: `https://raw.githubusercontent.com/libyal/libfsapfs/f179325e5405d3b09a314348646e9898b722759f/tests/fsapfs_test_compressed_data_handle.c`
  - License: LGPL-3.0-or-later
  - Meaning: original binary image of a real `decmpfs` type-`7` xattr containing inline LZVN payload

The ASCII hex companion files remain easy to inspect in code review. The
matching binary images in `bin/` are what the e2e tests consume directly.

# lzvn

`lzvn` is a safe, clean-room Rust implementation of Apple's LZVN format.

Current scope:

- raw LZVN stream encoding
- raw LZVN stream decoding
- incremental raw encoder and decoder state machines
- Apple `bvxn` block encoding
- Apple `bvxn` block decoding
- incremental `bvxn` encoder and decoder state machines
- Apple `decmpfs` xattr and resource-fork encode/decode helpers for LZVN
- no `unsafe` code
- Apache-2.0 licensing

Planned next steps:

- higher-level APFS / HFS+ integration helpers
- fuzzing, benchmarks, and differential tests against Apple-compatible output

Validation:

- `cargo test`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo bench`

Fixtures:

- real-world regression fixtures live in `tests/fixtures/`
- source urls and license notes are documented in `tests/fixtures/README.md`

Example:

```rust
let encoded = lzvn::encode_raw(b"hello");
let decoded = lzvn::decode_raw(&encoded, 5)?;

assert_eq!(decoded, b"hello");
# Ok::<(), lzvn::Error>(())
```

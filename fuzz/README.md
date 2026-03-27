# Fuzzing

Install the runner:

- `cargo install cargo-fuzz`

Targets:

- `cargo fuzz run raw_decode`
- `cargo fuzz run bvxn_decode`
- `cargo fuzz run decmpfs_decode`
- `cargo fuzz run roundtrip`

Seed corpora live in `fuzz/corpus/` and include real-world fixture-derived
inputs for raw LZVN, `bvxn`, and `decmpfs` coverage.

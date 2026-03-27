//! Criterion benchmarks for raw, `bvxn`, and `decmpfs` paths.
#![allow(missing_docs)]

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use lzvn::{decode_bvxn, decode_decmpfs, decode_raw, encode_bvxn, encode_decmpfs, encode_raw};

fn repetitive_bytes(len: usize) -> Vec<u8> {
  vec![b'a'; len]
}

fn text_bytes(len: usize) -> Vec<u8> {
  let seed = b"lorem ipsum dolor sit amet, consectetur adipiscing elit. ";
  let mut bytes = Vec::with_capacity(len);
  while bytes.len() < len {
    let remaining = len - bytes.len();
    let take = remaining.min(seed.len());
    bytes.extend_from_slice(&seed[..take]);
  }
  bytes
}

fn pseudo_random_bytes(len: usize) -> Vec<u8> {
  let mut state = 0x1234_5678_u32;
  let mut bytes = Vec::with_capacity(len);
  for _ in 0..len {
    state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    bytes.push((state >> 24) as u8);
  }
  bytes
}

fn raw_benchmarks(c: &mut Criterion) {
  let mut group = c.benchmark_group("raw");
  let datasets = [
    ("repetitive-4k", repetitive_bytes(4_096)),
    ("text-4k", text_bytes(4_096)),
    ("random-4k", pseudo_random_bytes(4_096)),
    ("random-64k", pseudo_random_bytes(65_536)),
  ];

  for (name, data) in &datasets {
    let encoded = encode_raw(data);
    group.bench_with_input(BenchmarkId::new("encode", name), data, |b, input| {
      b.iter(|| black_box(encode_raw(black_box(input))))
    });
    group.bench_with_input(
      BenchmarkId::new("decode", name),
      &(encoded.as_slice(), data.len()),
      |b, (input, decoded_len)| {
        b.iter(|| {
          black_box(decode_raw(black_box(input), *decoded_len).expect("raw decode should succeed"))
        })
      },
    );
  }

  group.finish();
}

fn bvxn_benchmarks(c: &mut Criterion) {
  let mut group = c.benchmark_group("bvxn");
  let datasets = [
    ("repetitive-4k", repetitive_bytes(4_096)),
    ("text-4k", text_bytes(4_096)),
    ("random-4k", pseudo_random_bytes(4_096)),
  ];

  for (name, data) in &datasets {
    let encoded = encode_bvxn(data).expect("bvxn encode should succeed");
    group.bench_with_input(BenchmarkId::new("encode", name), data, |b, input| {
      b.iter(|| black_box(encode_bvxn(black_box(input)).expect("bvxn encode should succeed")))
    });
    group.bench_with_input(BenchmarkId::new("decode", name), &encoded, |b, input| {
      b.iter(|| black_box(decode_bvxn(black_box(input)).expect("bvxn decode should succeed")))
    });
  }

  group.finish();
}

fn decmpfs_benchmarks(c: &mut Criterion) {
  let mut group = c.benchmark_group("decmpfs");
  let datasets = [
    ("inline-repetitive-4k", repetitive_bytes(4_096)),
    ("inline-random-2k", pseudo_random_bytes(2_048)),
    ("resource-random-70k", pseudo_random_bytes(70_000)),
  ];

  for (name, data) in &datasets {
    let encoded = encode_decmpfs(data).expect("decmpfs encode should succeed");
    group.bench_with_input(BenchmarkId::new("encode", name), data, |b, input| {
      b.iter(|| black_box(encode_decmpfs(black_box(input)).expect("decmpfs encode should succeed")))
    });
    group.bench_with_input(BenchmarkId::new("decode", name), &encoded, |b, input| {
      b.iter(|| {
        black_box(
          decode_decmpfs(
            black_box(&input.xattr),
            black_box(input.resource_fork.as_deref()),
          )
          .expect("decmpfs decode should succeed"),
        )
      })
    });
  }

  group.finish();
}

criterion_group!(benches, raw_benchmarks, bvxn_benchmarks, decmpfs_benchmarks);
criterion_main!(benches);

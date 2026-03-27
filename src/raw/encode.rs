const CANDIDATES_PER_BUCKET: usize = 4;
const EOS: [u8; 8] = [0x06, 0, 0, 0, 0, 0, 0, 0];
const HASH_BITS: usize = 14;
const HASH_SIZE: usize = 1 << HASH_BITS;
const INVALID_POSITION: usize = usize::MAX;
const MAX_DISTANCE: usize = 0xFFFF;
const MAX_INLINE_LITERAL: usize = 3;
const MAX_LITERAL_LEN: usize = 271;
const MAX_MATCH_LEN: usize = 271;
const MAX_MEDIUM_DISTANCE: usize = 0x3FFF;
const MAX_MEDIUM_MATCH: usize = 34;
const MAX_SHORT_DISTANCE: usize = 0x600;
const MIN_MATCH_LEN: usize = 3;

/// Encode bytes as a raw LZVN stream.
///
/// The returned stream includes the padded 8-byte end marker emitted by
/// Apple-compatible encoders.
///
/// # Examples
///
/// ```rust
/// let encoded = lzvn::raw::encode(b"abcabcabc");
/// let decoded = lzvn::raw::decode(&encoded, 9)?;
///
/// assert_eq!(decoded, b"abcabcabc");
/// # Ok::<(), lzvn::Error>(())
/// ```
pub fn encode(src: &[u8]) -> Vec<u8> {
  Encoder::new(src).finish()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Match {
  distance: usize,
  length: usize,
}

struct Encoder<'a> {
  src: &'a [u8],
  output: Vec<u8>,
  table: Vec<[usize; CANDIDATES_PER_BUCKET]>,
  literal_start: usize,
  position: usize,
  previous_distance: usize,
}

impl<'a> Encoder<'a> {
  fn new(src: &'a [u8]) -> Self {
    let capacity = src.len() + (src.len() / 16) + EOS.len();
    Self {
      src,
      output: Vec::with_capacity(capacity),
      table: vec![[INVALID_POSITION; CANDIDATES_PER_BUCKET]; HASH_SIZE],
      literal_start: 0,
      position: 0,
      previous_distance: 0,
    }
  }

  fn finish(mut self) -> Vec<u8> {
    while self.position < self.src.len() {
      let Some(next_match) = self.find_match(self.position) else {
        self.insert_position(self.position);
        self.position += 1;
        continue;
      };

      self.emit_match(next_match);

      let match_end = self.position + next_match.length;
      for index in self.position..match_end {
        self.insert_position(index);
      }

      self.position = match_end;
      self.literal_start = match_end;
    }

    let literal_start = self.literal_start;
    let literal_end = self.src.len();
    self.emit_literals_from_source(literal_start, literal_end);
    self.output.extend_from_slice(&EOS);
    self.output
  }

  fn find_match(&self, position: usize) -> Option<Match> {
    if self.src.len().saturating_sub(position) < MIN_MATCH_LEN {
      return None;
    }

    let mut best = None;

    if self.previous_distance != 0 && position >= self.previous_distance {
      self.consider_candidate(position, position - self.previous_distance, &mut best);
    }

    let bucket = self.hash_at(position);
    for candidate in self.table[bucket] {
      if candidate == INVALID_POSITION || candidate >= position {
        continue;
      }

      self.consider_candidate(position, candidate, &mut best);
    }

    let found = best?;
    if self.match_is_worthwhile(found) {
      Some(found)
    } else {
      None
    }
  }

  fn consider_candidate(&self, position: usize, candidate: usize, best: &mut Option<Match>) {
    let distance = position - candidate;
    if distance == 0 || distance > MAX_DISTANCE {
      return;
    }

    if self.src[candidate] != self.src[position]
      || self.src[candidate + 1] != self.src[position + 1]
      || self.src[candidate + 2] != self.src[position + 2]
    {
      return;
    }

    let next = Match {
      distance,
      length: self.match_length(candidate, position),
    };

    if next.length < MIN_MATCH_LEN {
      return;
    }

    if Self::is_better_match(next, *best, self.previous_distance) {
      *best = Some(next);
    }
  }

  fn is_better_match(candidate: Match, current: Option<Match>, previous_distance: usize) -> bool {
    let Some(current) = current else {
      return true;
    };

    if candidate.length != current.length {
      return candidate.length > current.length;
    }

    let candidate_prev = candidate.distance == previous_distance;
    let current_prev = current.distance == previous_distance;
    if candidate_prev != current_prev {
      return candidate_prev;
    }

    candidate.distance < current.distance
  }

  fn match_is_worthwhile(&self, next_match: Match) -> bool {
    if next_match.distance == self.previous_distance || next_match.distance < MAX_SHORT_DISTANCE {
      return next_match.length >= MIN_MATCH_LEN;
    }

    next_match.length > MIN_MATCH_LEN
  }

  fn match_length(&self, candidate: usize, position: usize) -> usize {
    let max_length = self.src.len() - position;
    let mut length = 0;

    while length < max_length && self.src[candidate + length] == self.src[position + length] {
      length += 1;
    }

    length
  }

  fn hash_at(&self, position: usize) -> usize {
    let value = (self.src[position] as u32)
      | ((self.src[position + 1] as u32) << 8)
      | ((self.src[position + 2] as u32) << 16);
    let hash = (value.wrapping_mul(1 + (1 << 6) + (1 << 12))) >> 12;
    (hash as usize) & (HASH_SIZE - 1)
  }

  fn insert_position(&mut self, position: usize) {
    if self.src.len().saturating_sub(position) < MIN_MATCH_LEN {
      return;
    }

    let bucket = self.hash_at(position);
    let slot = &mut self.table[bucket];
    slot.rotate_right(1);
    slot[0] = position;
  }

  fn emit_match(&mut self, next_match: Match) {
    let literal_start = self.literal_start;
    let literal_end = self.position;
    self.emit_match_with_literals(
      &self.src[literal_start..literal_end],
      next_match.length,
      next_match.distance,
    );
  }

  fn emit_match_with_literals(&mut self, literals: &[u8], mut match_len: usize, distance: usize) {
    let inline_start = literals.len().saturating_sub(MAX_INLINE_LITERAL);
    self.emit_literals(&literals[..inline_start]);

    let inline_literals = &literals[inline_start..];
    let inline_len = inline_literals.len();
    let short_match_limit = max_short_match(inline_len);

    if distance == self.previous_distance && inline_len == 0 {
      self.emit_match_chunks(match_len);
      return;
    }

    if distance == self.previous_distance {
      let initial_match = match_len.min(short_match_limit);
      self.emit_previous_distance(inline_literals, initial_match);
      match_len -= initial_match;
      self.previous_distance = distance;
      self.emit_match_chunks(match_len);
      return;
    }

    if distance <= MAX_MEDIUM_DISTANCE && match_len > short_match_limit {
      let initial_match = match_len.min(MAX_MEDIUM_MATCH);
      self.emit_medium_distance(inline_literals, initial_match, distance);
      match_len -= initial_match;
    } else if distance < MAX_SHORT_DISTANCE {
      let initial_match = match_len.min(short_match_limit);
      self.emit_small_distance(inline_literals, initial_match, distance);
      match_len -= initial_match;
    } else {
      let initial_match = match_len.min(short_match_limit);
      self.emit_large_distance(inline_literals, initial_match, distance);
      match_len -= initial_match;
    }

    self.previous_distance = distance;
    self.emit_match_chunks(match_len);
  }

  fn emit_literals_from_source(&mut self, start: usize, end: usize) {
    self.emit_literals(&self.src[start..end]);
  }

  fn emit_literals(&mut self, mut literals: &[u8]) {
    while !literals.is_empty() {
      let chunk_len = if literals.len() <= 15 {
        literals.len()
      } else {
        literals.len().min(MAX_LITERAL_LEN)
      };

      if chunk_len <= 15 {
        self.output.push(0xE0 | chunk_len as u8);
      } else {
        self.output.push(0xE0);
        self.output.push((chunk_len - 16) as u8);
      }

      self.output.extend_from_slice(&literals[..chunk_len]);
      literals = &literals[chunk_len..];
    }
  }

  fn emit_small_distance(&mut self, literals: &[u8], match_len: usize, distance: usize) {
    debug_assert!(distance < MAX_SHORT_DISTANCE);
    debug_assert!((MIN_MATCH_LEN..=max_short_match(literals.len())).contains(&match_len));

    let opcode = ((literals.len() as u8) << 6)
      | (((match_len - MIN_MATCH_LEN) as u8) << 3)
      | ((distance >> 8) as u8);
    self.output.push(opcode);
    self.output.push(distance as u8);
    self.output.extend_from_slice(literals);
  }

  fn emit_medium_distance(&mut self, literals: &[u8], match_len: usize, distance: usize) {
    debug_assert!(distance <= MAX_MEDIUM_DISTANCE);
    debug_assert!((MIN_MATCH_LEN..=MAX_MEDIUM_MATCH).contains(&match_len));

    let match_code = (match_len - MIN_MATCH_LEN) as u8;
    let opcode = 0xA0 | ((literals.len() as u8) << 3) | (match_code >> 2);
    let packed = ((distance as u16) << 2) | (match_code as u16 & 0x03);

    self.output.push(opcode);
    self.output.extend_from_slice(&packed.to_le_bytes());
    self.output.extend_from_slice(literals);
  }

  fn emit_large_distance(&mut self, literals: &[u8], match_len: usize, distance: usize) {
    debug_assert!((MIN_MATCH_LEN..=max_short_match(literals.len())).contains(&match_len));

    let opcode = ((literals.len() as u8) << 6) | (((match_len - MIN_MATCH_LEN) as u8) << 3) | 7;
    self.output.push(opcode);
    self
      .output
      .extend_from_slice(&(distance as u16).to_le_bytes());
    self.output.extend_from_slice(literals);
  }

  fn emit_previous_distance(&mut self, literals: &[u8], match_len: usize) {
    debug_assert!(self.previous_distance != 0);
    debug_assert!((MIN_MATCH_LEN..=max_short_match(literals.len())).contains(&match_len));

    let opcode = ((literals.len() as u8) << 6) | (((match_len - MIN_MATCH_LEN) as u8) << 3) | 6;
    self.output.push(opcode);
    self.output.extend_from_slice(literals);
  }

  fn emit_match_chunks(&mut self, mut match_len: usize) {
    while match_len > 0 {
      let chunk_len = if match_len <= 15 {
        match_len
      } else {
        match_len.min(MAX_MATCH_LEN)
      };

      if chunk_len <= 15 {
        self.output.push(0xF0 | chunk_len as u8);
      } else {
        self.output.push(0xF0);
        self.output.push((chunk_len - 16) as u8);
      }

      match_len -= chunk_len;
    }
  }
}

const fn max_short_match(inline_literals: usize) -> usize {
  10 - (inline_literals * 2)
}

#[cfg(test)]
mod tests {
  use super::Encoder;

  #[test]
  fn emits_previous_distance_with_followup_match() {
    let mut encoder = Encoder::new(b"");
    encoder.previous_distance = 3;
    encoder.emit_match_with_literals(b"abc", 6, 3);

    assert_eq!(encoder.output, [0xCE, b'a', b'b', b'c', 0xF2]);
  }

  #[test]
  fn emits_large_distance_with_followup_match() {
    let mut encoder = Encoder::new(b"");
    encoder.emit_match_with_literals(b"abc", 5, 20_000);

    assert_eq!(encoder.output, [0xCF, 0x20, 0x4E, b'a', b'b', b'c', 0xF1]);
  }
}

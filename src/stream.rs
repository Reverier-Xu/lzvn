/// Status returned by an incremental encoder or decoder step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamStatus {
  /// More input is required before progress can continue.
  NeedInput,
  /// More output space is required before progress can continue.
  NeedOutput,
  /// The stream has completed successfully.
  Finished,
}

/// Progress information returned by an incremental encoder or decoder step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamProgress {
  /// The number of bytes written into the caller-provided output buffer.
  pub written: usize,
  /// The next action required to continue processing.
  pub status: StreamStatus,
}

impl StreamProgress {
  pub(crate) const fn new(written: usize, status: StreamStatus) -> Self {
    Self { written, status }
  }
}

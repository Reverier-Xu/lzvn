//! Raw LZVN stream encoding and decoding.
//!
//! A raw LZVN stream is an opcode stream terminated by an end marker. Apple
//! encoders commonly emit an 8-byte padded form, while decoders may also
//! encounter compact single-byte `0x06` end markers in the wild.
//! Unlike Apple container formats, it does not store the decoded size.

mod decode;
mod encode;
mod opcode;
mod stream;

pub use decode::{decode, decode_into};
pub use encode::encode;
pub use stream::{RawDecoder, RawEncoder};

//! Coredump discovery: directory scans, journal matching, bounded streams.

pub mod dir_scanner;
pub mod journal_matcher;
pub mod stream_reader;
pub mod xattr_reader;

pub use dir_scanner::find_latest_coredump;
pub use journal_matcher::match_coredump_record;
pub use stream_reader::{lz4_flex, open_bounded_decompressed_reader, read_bounded_coredump_bytes};
pub use stream_reader::{BoundedStreamReader, MAX_DECOMPRESSED_BYTES};
pub use xattr_reader::read_coredump_xattrs;

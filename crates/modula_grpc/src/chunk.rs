//! Shared chunking for size-safe server streams. Large JSON payloads
//! (diffs, snapshots) are split into fixed-size pieces so a single message
//! never approaches tonic's 4 MB decode cap; clients concatenate the bytes.

/// Per-chunk payload size. Stays well under tonic's 4 MB default decode cap
/// while keeping the chunk count low for typical payloads.
pub const CHUNK_SIZE: usize = 256 * 1024;

/// Split an assembled byte payload into [`CHUNK_SIZE`] pieces. Empty input
/// yields no chunks (an empty stream).
pub fn split(data: Vec<u8>) -> Vec<Vec<u8>> {
    if data.is_empty() {
        return Vec::new();
    }
    data.chunks(CHUNK_SIZE).map(<[u8]>::to_vec).collect()
}

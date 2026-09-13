//! Plain-text extraction with automatic encoding detection.
//!
//! Uses `chardetng` to guess the encoding of files that are not UTF-8/UTF-16,
//! which matters for legacy GBK-era Chinese documents found on intranet
//! machines.

use super::MAX_EXTRACT_BYTES;
use std::io::Read;
use std::path::Path;

/// Read a text file, honouring BOMs and falling back to encoding detection.
pub fn read_text(path: &Path) -> Option<String> {
    // Bound the read itself: metadata size and on-disk size can diverge (sparse
    // files, races), and the caller's metadata gate must never be the only
    // defence against an oversized file.
    let file = std::fs::File::open(path).ok()?;
    let mut bytes = Vec::new();
    file.take(MAX_EXTRACT_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() as u64 > MAX_EXTRACT_BYTES {
        return None;
    }
    if bytes.is_empty() {
        return None;
    }

    // BOM-based encodings take priority.
    if let Some((enc, bom_len)) = encoding_rs::Encoding::for_bom(&bytes) {
        let (cow, _, _) = enc.decode(&bytes[bom_len..]);
        return Some(cow.into_owned());
    }

    // Fast path: valid UTF-8.
    if let Ok(s) = std::str::from_utf8(&bytes) {
        return Some(s.to_string());
    }

    // Guess a legacy encoding (e.g. GBK/GB18030 on Chinese intranets).
    let mut detector = chardetng::EncodingDetector::new();
    detector.feed(&bytes, true);
    let enc = detector.guess(None, true);
    let (cow, _, _) = enc.decode(&bytes);
    let text = cow.into_owned();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

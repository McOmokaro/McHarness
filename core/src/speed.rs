//! The speed organs.
//!
//! Historically these three hot paths were intended to live in C++ behind a
//! `#[link(name = "mcharness_speed")]` FFI boundary. Until that native library
//! is built and shipped, this is a pure-Rust fallback with the exact same safe
//! signatures, so the rest of the core consumes them unchanged.

/// Push a token into `out`, returning the number of bytes written.
pub fn stream_push(token: &str, out: &mut [u8]) -> usize {
    let bytes = token.as_bytes();
    let n = bytes.len().min(out.len());
    out[..n].copy_from_slice(&bytes[..n]);
    n
}

/// Fast memory search: the index of the first entry containing `query`.
pub fn memsearch(query: &str, entries: &[&str]) -> Option<usize> {
    if query.is_empty() {
        return None;
    }
    entries.iter().position(|e| e.contains(query))
}

/// Cosine similarity between two equal-length embeddings.
pub fn cosine(a: &[f32], b: &[f32]) -> f64 {
    assert_eq!(a.len(), b.len(), "embedding dims must match");
    let mut dot = 0.0f64;
    let mut na = 0.0f64;
    let mut nb = 0.0f64;
    for (x, y) in a.iter().zip(b.iter()) {
        let (x, y) = (*x as f64, *y as f64);
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    dot / (na.sqrt() * nb.sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_copies_and_caps() {
        let mut buf = [0u8; 4];
        assert_eq!(stream_push("ab", &mut buf), 2);
        assert_eq!(&buf[..2], b"ab");
        assert_eq!(stream_push("abcdef", &mut buf), 4);
    }

    #[test]
    fn memsearch_finds_containing_entry() {
        assert_eq!(memsearch("llama", &["qwen2.5", "llama3.1"]), Some(1));
        assert_eq!(memsearch("nope", &["a", "b"]), None);
        assert_eq!(memsearch("", &["a"]), None);
    }

    #[test]
    fn cosine_identical_is_one() {
        let v = [1.0f32, 2.0, 3.0];
        assert!((cosine(&v, &v) - 1.0).abs() < 1e-9);
        assert_eq!(cosine(&[0.0, 0.0], &[1.0, 1.0]), 0.0);
    }
}

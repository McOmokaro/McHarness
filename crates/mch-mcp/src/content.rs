//! Content handling for resources returned by MCP servers.
//!
//! Forged from Goose `crates/goose-provider-types/src/mcp_utils.rs`. Content
//! from any MCP server is untrusted input: it can carry hidden instructions.

use serde_json::Value;

/// Strip the Unicode "tag" block (U+E0000–U+E007F).
///
/// This is the Trojan-Source channel: a document that reads one way to a human
/// and another to the agent. McHarness treats resource text as data, never as
/// instructions, and this removes the channel that would blur the line.
pub fn sanitize_unicode_tags(input: &str) -> String {
    input
        .chars()
        .filter(|c| !(0xE0000..=0xE007F).contains(&(*c as u32)))
        .collect()
}

pub fn text_resource(text: &str) -> String {
    sanitize_unicode_tags(text)
}

/// A blob resource: decode base64 and return text if it is UTF-8, else a
/// placeholder naming the mime type and byte length.
pub fn blob_resource(blob_b64: &str, mime: Option<&str>) -> String {
    use base64_decode::decode_base64;

    match decode_base64(blob_b64) {
        Some(bytes) => match String::from_utf8(bytes) {
            Ok(text) => sanitize_unicode_tags(&text),
            Err(err) => {
                let mime = mime.unwrap_or("application/octet-stream");
                format!("[Binary content ({}) - {} bytes]", mime, err.into_bytes().len())
            }
        },
        None => sanitize_unicode_tags(blob_b64),
    }
}

/// Extract readable text from a resource contents JSON value.
pub fn extract_text(value: &Value) -> String {
    if let Some(text) = value.get("text").and_then(Value::as_str) {
        return sanitize_unicode_tags(text);
    }
    if let Some(blob) = value.get("blob").and_then(Value::as_str) {
        let mime = value.get("mimeType").and_then(Value::as_str);
        return blob_resource(blob, mime);
    }
    String::new()
}

/// Minimal dependency-free base64 (standard alphabet, padding required).
mod base64_decode {
    pub fn decode_base64(input: &str) -> Option<Vec<u8>> {
        fn val(c: u8) -> Option<u8> {
            match c {
                b'A'..=b'Z' => Some(c - b'A'),
                b'a'..=b'z' => Some(c - b'a' + 26),
                b'0'..=b'9' => Some(c - b'0' + 52),
                b'+' => Some(62),
                b'/' => Some(63),
                _ => None,
            }
        }
        let input = input.trim();
        if input.len() % 4 != 0 {
            return None;
        }
        let mut out = Vec::with_capacity(input.len() / 4 * 3);
        for chunk in input.as_bytes().chunks(4) {
            let p = chunk.iter().filter(|&&c| c != b'=').count();
            let mut buf = [0u8; 4];
            for (i, &c) in chunk.iter().enumerate() {
                buf[i] = if c == b'=' { 0 } else { val(c)? };
            }
            let n = (buf[0] as u32) << 18
                | (buf[1] as u32) << 12
                | (buf[2] as u32) << 6
                | (buf[3] as u32);
            out.push((n >> 16) as u8);
            if p >= 3 {
                out.push((n >> 8) as u8);
            }
            if p >= 4 {
                out.push(n as u8);
            }
        }
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_unicode_tags() {
        let input = "visible\u{E0041}\u{E0042}text";
        assert_eq!(sanitize_unicode_tags(input), "visibletext");
    }

    #[test]
    fn text_resource_passthrough() {
        let v = serde_json::json!({ "text": "hello" });
        assert_eq!(extract_text(&v), "hello");
    }

    #[test]
    fn blob_utf8_decodes() {
        // "hi" == aGk=
        let v = serde_json::json!({ "blob": "aGk=", "mimeType": "text/plain" });
        assert_eq!(extract_text(&v), "hi");
    }

    #[test]
    fn blob_binary_placeholder() {
        // 0xFF 0xFE -> //4= base64 of invalid utf8
        let v = serde_json::json!({ "blob": "//4=", "mimeType": "image/png" });
        let out = extract_text(&v);
        assert!(out.starts_with("[Binary content (image/png)"), "got: {out}");
    }
}

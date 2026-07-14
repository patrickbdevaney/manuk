//! `multipart/form-data` encoding (L05 uploads): the wire format a browser sends when a form
//! with a file input (or `enctype="multipart/form-data"`) is submitted (RFC 7578).
//!
//! Pure + deterministic — the boundary is supplied by the caller, so the exact bytes are
//! testable. The shell builds [`Part`]s from a form's fields + the user's chosen files and POSTs
//! `encode(...)` via [`crate::request`].

/// One part of a multipart body: a plain field (`filename`/`content_type` = `None`) or a file
/// (both `Some`).
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    /// The form control's `name`.
    pub name: String,
    /// The uploaded file's name, for a file part; `None` for a plain field.
    pub filename: Option<String>,
    /// The part's `Content-Type` (e.g. `text/plain`), for a file part; `None` for a plain field.
    pub content_type: Option<String>,
    /// The part's payload bytes (a field value's UTF-8, or the file's contents).
    pub body: Vec<u8>,
}

impl Part {
    /// A plain form field.
    pub fn field(name: impl Into<String>, value: impl Into<String>) -> Part {
        Part {
            name: name.into(),
            filename: None,
            content_type: None,
            body: value.into().into_bytes(),
        }
    }

    /// A file upload part.
    pub fn file(
        name: impl Into<String>,
        filename: impl Into<String>,
        content_type: impl Into<String>,
        body: Vec<u8>,
    ) -> Part {
        Part {
            name: name.into(),
            filename: Some(filename.into()),
            content_type: Some(content_type.into()),
            body,
        }
    }
}

/// The `Content-Type` header value for a multipart body with the given `boundary`.
pub fn content_type(boundary: &str) -> String {
    format!("multipart/form-data; boundary={boundary}")
}

/// Encode `parts` into a `multipart/form-data` body delimited by `boundary`. Returns
/// `(content_type_header_value, body_bytes)`. The caller must ensure `boundary` does not occur in
/// any part body (browsers pick a long random token; [`crate::multipart::random_boundary`] gives
/// one).
///
/// Header names within a part are ASCII; a `filename`/`name` containing `"` or newlines is
/// escaped/stripped so it cannot break the header grammar (a mild interpretation of RFC 7578 §5.1
/// — enough to be safe, not full RFC 2231).
pub fn encode(parts: &[Part], boundary: &str) -> (String, Vec<u8>) {
    let mut body: Vec<u8> = Vec::new();
    for part in parts {
        body.extend_from_slice(b"--");
        body.extend_from_slice(boundary.as_bytes());
        body.extend_from_slice(b"\r\n");

        body.extend_from_slice(b"Content-Disposition: form-data; name=\"");
        body.extend_from_slice(header_escape(&part.name).as_bytes());
        body.extend_from_slice(b"\"");
        if let Some(fname) = &part.filename {
            body.extend_from_slice(b"; filename=\"");
            body.extend_from_slice(header_escape(fname).as_bytes());
            body.extend_from_slice(b"\"");
        }
        body.extend_from_slice(b"\r\n");

        if let Some(ct) = &part.content_type {
            body.extend_from_slice(b"Content-Type: ");
            body.extend_from_slice(header_escape(ct).as_bytes());
            body.extend_from_slice(b"\r\n");
        }
        body.extend_from_slice(b"\r\n");
        body.extend_from_slice(&part.body);
        body.extend_from_slice(b"\r\n");
    }
    // Closing delimiter.
    body.extend_from_slice(b"--");
    body.extend_from_slice(boundary.as_bytes());
    body.extend_from_slice(b"--\r\n");

    (content_type(boundary), body)
}

/// Strip characters that would break the `Content-Disposition` header grammar (quotes, CR, LF).
fn header_escape(s: &str) -> String {
    s.chars()
        .filter(|&c| c != '"' && c != '\r' && c != '\n')
        .collect()
}

/// A boundary token unlikely to collide with body content, derived from a caller-supplied `seed`
/// (a counter or hash) — kept out of `encode` so encoding stays pure/deterministic. Callers that
/// want randomness pass a random seed; tests pass a fixed one.
pub fn boundary_from_seed(seed: u64) -> String {
    format!("----ManukFormBoundary{seed:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_a_field_and_a_file_exactly() {
        let parts = vec![
            Part::field("title", "Hello"),
            Part::file("doc", "a.txt", "text/plain", b"FILE-BODY".to_vec()),
        ];
        let (ct, body) = encode(&parts, "BOUND");
        assert_eq!(ct, "multipart/form-data; boundary=BOUND");
        let expected = "--BOUND\r\n\
             Content-Disposition: form-data; name=\"title\"\r\n\
             \r\n\
             Hello\r\n\
             --BOUND\r\n\
             Content-Disposition: form-data; name=\"doc\"; filename=\"a.txt\"\r\n\
             Content-Type: text/plain\r\n\
             \r\n\
             FILE-BODY\r\n\
             --BOUND--\r\n";
        assert_eq!(String::from_utf8(body).unwrap(), expected);
    }

    #[test]
    fn field_names_cannot_break_the_header() {
        let parts = vec![Part::file(
            "x\"y",
            "e\"vil\r\n.txt",
            "text/plain",
            b"z".to_vec(),
        )];
        let (_, body) = encode(&parts, "B");
        let s = String::from_utf8(body).unwrap();
        assert!(
            s.contains("name=\"xy\"; filename=\"evil.txt\""),
            "quotes/newlines stripped: {s}"
        );
        // Exactly one part → exactly one Content-Disposition line.
        assert_eq!(s.matches("Content-Disposition").count(), 1);
    }

    #[test]
    fn empty_parts_still_closes() {
        let (_, body) = encode(&[], "B");
        assert_eq!(String::from_utf8(body).unwrap(), "--B--\r\n");
    }

    #[test]
    fn boundary_seed_is_deterministic() {
        assert_eq!(boundary_from_seed(1), boundary_from_seed(1));
        assert_ne!(boundary_from_seed(1), boundary_from_seed(2));
        assert!(boundary_from_seed(0xABCD).starts_with("----ManukFormBoundary"));
    }
}

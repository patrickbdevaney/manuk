//! §4b — HTML form model: find the form around a control, read its fields, and build
//! the URL a **GET** submission would navigate to.
//!
//! **Documented gaps (not faked):** only `method=get` is implemented. `POST` bodies
//! are *not* silently downgraded to GET — that would send credentials in a URL and in
//! the referrer. [`submission_url`] returns [`SubmitError::PostUnsupported`] instead,
//! and the agent surfaces it. `<select multiple>`, `<input type=file>`, `formaction`
//! overrides, and `enctype` are not modelled.

use manuk_dom::{Dom, NodeId};
use url::Url;

#[derive(Debug, PartialEq, Eq)]
pub enum SubmitError {
    /// The control is not inside a `<form>` and the document has none.
    NoForm,
    /// `method="post"` — deliberately refused rather than downgraded to GET.
    PostUnsupported,
    /// The form's `action` could not be resolved against the page URL.
    BadAction(String),
}

impl std::fmt::Display for SubmitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SubmitError::NoForm => write!(f, "no enclosing <form>"),
            SubmitError::PostUnsupported => write!(
                f,
                "form method=post is not implemented (refusing to downgrade to GET, \
                 which would leak field values into the URL)"
            ),
            SubmitError::BadAction(a) => write!(f, "cannot resolve form action {a:?}"),
        }
    }
}

/// The nearest `<form>` ancestor of `node` (inclusive), else the document's first form.
pub fn owning_form(dom: &Dom, node: NodeId) -> Option<NodeId> {
    let mut cur = Some(node);
    while let Some(n) = cur {
        if dom.tag_name(n) == Some("form") {
            return Some(n);
        }
        cur = dom.parent(n);
    }
    dom.find_first("form")
}

/// Whether a checkbox/radio is checked (the `checked` content attribute).
fn is_checked(dom: &Dom, n: NodeId) -> bool {
    dom.element(n).is_some_and(|e| e.attr("checked").is_some())
}

fn input_type(dom: &Dom, n: NodeId) -> String {
    dom.element(n)
        .and_then(|e| e.attr("type"))
        .unwrap_or("text")
        .to_ascii_lowercase()
}

/// The successful controls of `form`, in document order, as `(name, value)` pairs —
/// i.e. what a browser would serialize on submit (HTML §form-submission).
pub fn fields(dom: &Dom, form: NodeId) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for n in dom.descendants(form) {
        let Some(el) = dom.element(n) else { continue };
        // A control without a `name` is never successful.
        let Some(name) = el.attr("name").filter(|s| !s.is_empty()) else {
            continue;
        };
        let name = name.to_string();

        match el.name.as_str() {
            "input" => match input_type(dom, n).as_str() {
                // Buttons only submit their own value when they are the activating
                // control; we do not model that, so they are skipped.
                "submit" | "button" | "reset" | "image" => continue,
                // File uploads cannot be represented in a GET query.
                "file" => continue,
                "checkbox" | "radio" => {
                    if is_checked(dom, n) {
                        // An `on` default matches the HTML spec.
                        out.push((name, el.attr("value").unwrap_or("on").to_string()));
                    }
                }
                _ => out.push((name, el.attr("value").unwrap_or_default().to_string())),
            },
            // A `<textarea>`'s submitted value is its text content — unless the agent
            // typed into it, which records a `value` attribute (we do not rewrite text
            // children). That attribute then wins, as the user's edit would.
            "textarea" => out.push((
                name,
                dom.element(n)
                    .and_then(|e| e.attr("value"))
                    .map(str::to_string)
                    .unwrap_or_else(|| dom.text_content(n)),
            )),
            "select" => {
                let mut chosen: Option<String> = None;
                let mut first: Option<String> = None;
                for opt in dom.descendants(n) {
                    if dom.tag_name(opt) != Some("option") {
                        continue;
                    }
                    let val = dom
                        .element(opt)
                        .and_then(|e| e.attr("value"))
                        .map(str::to_string)
                        .unwrap_or_else(|| dom.text_content(opt).trim().to_string());
                    if first.is_none() {
                        first = Some(val.clone());
                    }
                    if dom
                        .element(opt)
                        .is_some_and(|e| e.attr("selected").is_some())
                    {
                        chosen = Some(val);
                        break;
                    }
                }
                // With no explicit `selected`, a single-select picks its first option.
                if let Some(v) = chosen.or(first) {
                    out.push((name, v));
                }
            }
            _ => {}
        }
    }
    out
}

/// The absolute URL a **GET** submission of `form` navigates to, resolved against
/// `base` (the page URL). Existing query params on `action` are replaced, as browsers do.
pub fn submission_url(dom: &Dom, form: NodeId, base: &str) -> Result<String, SubmitError> {
    let el = dom.element(form).ok_or(SubmitError::NoForm)?;
    let method = el.attr("method").unwrap_or("get").to_ascii_lowercase();
    if method == "post" {
        return Err(SubmitError::PostUnsupported);
    }

    let action = el.attr("action").filter(|a| !a.trim().is_empty());
    let base_url = Url::parse(base).map_err(|_| SubmitError::BadAction(base.to_string()))?;
    let mut url = match action {
        Some(a) => base_url
            .join(a.trim())
            .map_err(|_| SubmitError::BadAction(a.to_string()))?,
        // No action => submit to the page's own URL.
        None => base_url,
    };

    let pairs = fields(dom, form);
    url.query_pairs_mut()
        .clear()
        .extend_pairs(pairs.iter().map(|(k, v)| (k, v)));
    // An empty form yields `?` from `query_pairs_mut`; strip it.
    if url.query() == Some("") {
        url.set_query(None);
    }
    Ok(url.to_string())
}

/// A ready-to-send `multipart/form-data` POST built from a form + the user's chosen files.
#[derive(Clone, Debug, PartialEq)]
pub struct MultipartPost {
    /// Absolute action URL (resolved against the page).
    pub url: String,
    /// `multipart/form-data; boundary=…` for the `Content-Type` header.
    pub content_type: String,
    /// The encoded request body.
    pub body: Vec<u8>,
}

/// The names of `type=file` controls in `form` that have a `name`, in document order — the inputs
/// a multipart submission expects file bytes for.
pub fn file_inputs(dom: &Dom, form: NodeId) -> Vec<String> {
    let mut out = Vec::new();
    for n in dom.descendants(form) {
        let Some(el) = dom.element(n) else { continue };
        if el.name == "input" && input_type(dom, n) == "file" {
            if let Some(name) = el.attr("name").filter(|s| !s.is_empty()) {
                out.push(name.to_string());
            }
        }
    }
    out
}

/// Build a `multipart/form-data` **POST** for `form` (L05 uploads): the successful non-file
/// controls (via [`fields`]) become plain parts, and each supplied
/// `(name, filename, content_type, bytes)` becomes a file part. The action resolves against
/// `base`; `boundary` is caller-supplied so the bytes are deterministic. Requires `method=post`
/// (a GET file upload is not a thing). The shell obtains the file bytes from the OS picker; a
/// headless caller supplies them directly.
pub fn multipart_submission(
    dom: &Dom,
    form: NodeId,
    base: &str,
    files: &[(String, String, String, Vec<u8>)],
    boundary: &str,
) -> Result<MultipartPost, SubmitError> {
    let el = dom.element(form).ok_or(SubmitError::NoForm)?;
    let method = el.attr("method").unwrap_or("get").to_ascii_lowercase();
    if method != "post" {
        return Err(SubmitError::BadAction(
            "multipart upload requires method=post".to_string(),
        ));
    }
    let action = el.attr("action").filter(|a| !a.trim().is_empty());
    let base_url = Url::parse(base).map_err(|_| SubmitError::BadAction(base.to_string()))?;
    let url = match action {
        Some(a) => base_url
            .join(a.trim())
            .map_err(|_| SubmitError::BadAction(a.to_string()))?,
        None => base_url,
    };

    let mut parts: Vec<manuk_net::multipart::Part> = fields(dom, form)
        .into_iter()
        .map(|(k, v)| manuk_net::multipart::Part::field(k, v))
        .collect();
    for (name, filename, ct, bytes) in files {
        parts.push(manuk_net::multipart::Part::file(
            name.clone(),
            filename.clone(),
            ct.clone(),
            bytes.clone(),
        ));
    }
    let (content_type, body) = manuk_net::multipart::encode(&parts, boundary);
    Ok(MultipartPost {
        url: url.to_string(),
        content_type,
        body,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dom_of(html: &str) -> Dom {
        manuk_html::parse(html)
    }

    #[test]
    fn collects_successful_controls_in_document_order() {
        let dom = dom_of(
            r#"<form action="/search">
                 <input name="q" value="rust">
                 <input name="ignored_no_value">
                 <input type="hidden" name="token" value="t1">
                 <input type="checkbox" name="safe" value="on" checked>
                 <input type="checkbox" name="unsafe" value="on">
                 <input type="submit" name="go" value="Go">
                 <input type="file" name="upload">
                 <textarea name="body">hello</textarea>
                 <select name="lang">
                   <option value="en">English</option>
                   <option value="fr" selected>French</option>
                 </select>
               </form>"#,
        );
        let form = dom.find_first("form").unwrap();
        let f = fields(&dom, form);
        assert_eq!(
            f,
            vec![
                ("q".into(), "rust".into()),
                ("ignored_no_value".into(), "".into()),
                ("token".into(), "t1".into()),
                ("safe".into(), "on".into()), // unchecked box omitted
                // submit + file omitted
                ("body".into(), "hello".into()),
                ("lang".into(), "fr".into()), // `selected` wins
            ]
        );
    }

    #[test]
    fn select_without_selected_picks_the_first_option() {
        let dom = dom_of(
            r#"<form><select name="c"><option value="a">A</option>
               <option value="b">B</option></select></form>"#,
        );
        let form = dom.find_first("form").unwrap();
        assert_eq!(fields(&dom, form), vec![("c".to_string(), "a".to_string())]);
    }

    #[test]
    fn get_submission_builds_an_encoded_query_against_the_base() {
        let dom = dom_of(r#"<form action="/search"><input name="q" value="a b&c"></form>"#);
        let form = dom.find_first("form").unwrap();
        let url = submission_url(&dom, form, "https://ex.test/dir/page").unwrap();
        assert_eq!(url, "https://ex.test/search?q=a+b%26c");
    }

    #[test]
    fn no_action_submits_to_the_page_itself_and_replaces_the_query() {
        let dom = dom_of(r#"<form><input name="q" value="new"></form>"#);
        let form = dom.find_first("form").unwrap();
        let url = submission_url(&dom, form, "https://ex.test/s?old=1").unwrap();
        assert_eq!(url, "https://ex.test/s?q=new");
    }

    #[test]
    fn empty_form_produces_no_dangling_question_mark() {
        let dom = dom_of(r#"<form action="/go"></form>"#);
        let form = dom.find_first("form").unwrap();
        assert_eq!(
            submission_url(&dom, form, "https://ex.test/").unwrap(),
            "https://ex.test/go"
        );
    }

    /// POST must NOT be silently downgraded to GET — that would put field values
    /// (passwords, tokens) into the URL and the referrer.
    #[test]
    fn post_is_refused_rather_than_downgraded_to_get() {
        let dom = dom_of(
            r#"<form method="POST" action="/login"><input name="pw" value="s3cr3t"></form>"#,
        );
        let form = dom.find_first("form").unwrap();
        assert_eq!(
            submission_url(&dom, form, "https://ex.test/"),
            Err(SubmitError::PostUnsupported)
        );
    }

    #[test]
    fn owning_form_walks_up_then_falls_back_to_the_first_form() {
        let dom =
            dom_of(r#"<form id="f"><div><button id="b">Go</button></div></form><p id="p">x</p>"#);
        let btn = dom.find_first("button").unwrap();
        let form = dom.find_first("form").unwrap();
        assert_eq!(owning_form(&dom, btn), Some(form));
        // A node outside any form falls back to the document's first form.
        let p = dom.find_first("p").unwrap();
        assert_eq!(owning_form(&dom, p), Some(form));
    }

    #[test]
    fn file_inputs_lists_named_file_controls() {
        let dom = dom_of(
            r#"<form><input type="file" name="a"><input type="file"><input name="b"><input type="file" name="c"></form>"#,
        );
        let form = dom.find_first("form").unwrap();
        assert_eq!(
            file_inputs(&dom, form),
            vec!["a".to_string(), "c".to_string()]
        );
    }

    #[test]
    fn multipart_post_builds_body_with_fields_and_file() {
        let dom = dom_of(
            r#"<form method="post" action="/upload"><input name="title" value="hi"><input type="file" name="doc"></form>"#,
        );
        let form = dom.find_first("form").unwrap();
        let files = vec![(
            "doc".to_string(),
            "a.txt".to_string(),
            "text/plain".to_string(),
            b"DATA".to_vec(),
        )];
        let post = multipart_submission(&dom, form, "https://ex.test/page", &files, "BOUND")
            .expect("post");
        assert_eq!(post.url, "https://ex.test/upload");
        assert_eq!(post.content_type, "multipart/form-data; boundary=BOUND");
        let s = String::from_utf8(post.body).unwrap();
        assert!(
            s.contains("name=\"title\"\r\n\r\nhi\r\n"),
            "text field part: {s}"
        );
        assert!(
            s.contains(
                "name=\"doc\"; filename=\"a.txt\"\r\nContent-Type: text/plain\r\n\r\nDATA\r\n"
            ),
            "file part: {s}"
        );
        assert!(s.ends_with("--BOUND--\r\n"), "closing delimiter");
    }

    #[test]
    fn multipart_requires_post() {
        let dom = dom_of(r#"<form action="/x"><input type="file" name="f"></form>"#);
        let form = dom.find_first("form").unwrap();
        assert!(multipart_submission(&dom, form, "https://ex.test/", &[], "B").is_err());
    }
}

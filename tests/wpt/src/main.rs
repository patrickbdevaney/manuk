//! `manuk-wpt` — run the conformance suite and report results.
//!
//! Today this runs the built-in layout reftests. When `$WPT_DIR` points at a
//! web-platform-tests checkout, that is where the upstream runner will iterate the
//! real `css/` and `dom/` suites.

use manuk_text::FontContext;

fn main() {
    let fonts = FontContext::new();
    if fonts.face_count() == 0 {
        eprintln!("note: no system fonts; text-dependent tests will be skipped");
    }

    let report = manuk_wpt::run_layout_suite(&fonts);
    print!("{}", report.summary());

    match manuk_wpt::find_wpt_checkout() {
        Some(dir) => eprintln!("WPT checkout: {} (upstream runner: TODO)", dir.display()),
        None => eprintln!("(set WPT_DIR to a web-platform-tests checkout to enable the upstream runner)"),
    }

    if !report.all_passed() {
        std::process::exit(1);
    }
}

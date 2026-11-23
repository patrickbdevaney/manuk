//! W6.D1.b: which backend paints the page the HUMAN looks at.
//!
//! W6.D1 applied the engine-ownership kill rule: Chrome (the pinned cdp-layer build) is the default
//! backend, so the window's default human view is a frame captured from Chrome. The in-process engine
//! view is opt-in (`--engine-view` or `MANUK_HUMAN_VIEW=engine`). Every [`Frame`] says which backend
//! produced it, and a Chrome frame carries the CDP target id, so a test (or a status line) can prove
//! which one the user is looking at.

use anyhow::{anyhow, Result};
use manuk_mcp::cdp::CdpBackend;
use manuk_mcp::Backend;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HumanView {
    Chrome,
    Engine,
}

impl HumanView {
    /// Default is Chrome. `args` may contain `--engine-view` / `--chrome-view`; the env var
    /// `MANUK_HUMAN_VIEW=engine|chrome` is the fallback. A flag beats the env var.
    pub fn from_config(args: &[String], env: Option<&str>) -> HumanView {
        if args.iter().any(|a| a == "--engine-view") {
            return HumanView::Engine;
        }
        if args.iter().any(|a| a == "--chrome-view") {
            return HumanView::Chrome;
        }
        match env {
            Some("engine") => HumanView::Engine,
            _ => HumanView::Chrome,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FrameSource {
    /// Captured from Chrome over CDP; the id of the page target.
    Chrome { target_id: String },
    Engine,
}

pub struct Frame {
    pub source: FrameSource,
    pub width: u32,
    pub height: u32,
    /// Straight RGBA8, row-major.
    pub rgba: Vec<u8>,
    /// The page's text as the same backend reports it.
    pub text: String,
}

impl Frame {
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.width + x) * 4) as usize;
        [self.rgba[i], self.rgba[i + 1], self.rgba[i + 2], self.rgba[i + 3]]
    }
}

/// A live Chrome the window shows. One per window; dropping it reaps Chrome.
pub struct ChromeView {
    be: CdpBackend,
}

impl ChromeView {
    pub fn launch(width: u32, height: u32) -> Result<Self> {
        Ok(ChromeView {
            be: CdpBackend::launch(width, height)?,
        })
    }

    pub fn navigate(&mut self, url: &str) -> Result<()> {
        manuk_net::runtime().block_on(async {
            self.be.navigate(url).await?;
            self.be.settle().await;
            Ok(())
        })
    }

    /// The current frame, captured from Chrome (`Page.captureScreenshot`).
    pub fn frame(&mut self) -> Result<Frame> {
        let png = self.be.screenshot_png()?;
        let img = image::load_from_memory_with_format(&png, image::ImageFormat::Png)
            .map_err(|e| anyhow!("Chrome frame is not a PNG: {e}"))?
            .to_rgba8();
        Ok(Frame {
            source: FrameSource::Chrome {
                target_id: self.be.target_id().to_string(),
            },
            width: img.width(),
            height: img.height(),
            rgba: img.into_raw(),
            text: self.be.page_text().unwrap_or_default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_chrome_and_engine_is_opt_in() {
        assert_eq!(HumanView::from_config(&[], None), HumanView::Chrome);
        assert_eq!(
            HumanView::from_config(&["--engine-view".into()], None),
            HumanView::Engine
        );
        assert_eq!(HumanView::from_config(&[], Some("engine")), HumanView::Engine);
        assert_eq!(
            HumanView::from_config(&["--chrome-view".into()], Some("engine")),
            HumanView::Chrome
        );
    }
}

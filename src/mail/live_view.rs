//! Interactive email view backed by a persistent Chromium page via CDP.
//!
//! The email is rendered exactly as before (real HTML/CSS, local images), but
//! because the page is live we can forward mouse drags into it so Chromium
//! performs genuine text selection, then screenshot the highlighted result
//! back into GPUI. Selected text is read with `Runtime.evaluate`.
//!
//! This gives the best of both worlds: pixel-perfect email rendering AND
//! real select/copy, for every email, with no per-mail special cases.

use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};

use super::fast_render::FastRenderer;
use super::render::build_document;
use crate::config;

/// A live Chromium page showing one email, with selection support.
pub struct LiveEmailView {
    renderer: FastRenderer,
    seq: u64,
    doc_path: PathBuf,
    height: u64,
}

impl LiveEmailView {
    pub fn start() -> Result<Self> {
        // Profiles from previous app runs are ~140 MB each and no longer
        // needed once their Chromium has exited.
        static CLEANED: std::sync::Once = std::sync::Once::new();
        CLEANED.call_once(|| {
            if let Ok(entries) = config::data_dir().read_dir() {
                for entry in entries.flatten() {
                    if entry.file_name().to_string_lossy().starts_with("live-profile-") {
                        let _ = std::fs::remove_dir_all(entry.path());
                    }
                }
            }
        });
        // Use a unique port/profile per view so rapid email switches don't
        // connect to a previous browser that is still shutting down.
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let port = 9400 + (id % 500) as u16;
        let profile = format!("live-profile-{}-{id}", std::process::id());
        let renderer = FastRenderer::start_with_port(port, &profile)?;
        let doc_path = config::data_dir()
            .join("email-render")
            .join(format!("live-{}.html", std::process::id()));
        Ok(Self {
            renderer,
            seq: 0,
            doc_path,
            height: 1000,
        })
    }

    /// Load an email's HTML into the live page, wait for it to paint, and
    /// remember its content height.
    pub fn load(&mut self, html: &str, base_url: Option<&str>) -> Result<()> {
        let document = build_document(html, base_url);
        if let Some(parent) = self.doc_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut file = std::fs::File::create(&self.doc_path)?;
        file.write_all(document.as_bytes())?;
        drop(file);

        let url = format!("file://{}", self.doc_path.display());
        {
            let client = self.renderer.client();
            let mut client = client.lock().map_err(|e| anyhow!("lock poisoned: {e}"))?;
            client.call("Page.enable", serde_json::json!({}))?;
            client.call(
                "Page.navigate",
                serde_json::json!({ "url": url }),
            )?;
        }

        // Wait until the document is ready AND every image has settled. Email
        // images are local file://, so this is fast (no remote hosts).
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let state = {
                let client = self.renderer.client();
                let mut client = client.lock().map_err(|e| anyhow!("lock poisoned: {e}"))?;
                client.call(
                    "Runtime.evaluate",
                    serde_json::json!({
                        "expression": "({ready:document.readyState, \
                         href:location.href, len:document.body?document.body.innerHTML.length:0, \
                         done:Array.from(document.images).every(i=>i.complete)})",
                        "returnByValue": true
                    }),
                )?
            };
            let value = state.get("result").and_then(|r| r.get("value"));
            let done = value
                .and_then(|v| v.get("done"))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true);
            let ready = value
                .and_then(|v| v.get("ready"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            let len = value
                .and_then(|v| v.get("len"))
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            let href = value
                .and_then(|v| v.get("href"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            let navigated = href.starts_with("file://") && len > 20;
            if (navigated && done && ready == "complete") || Instant::now() > deadline {
                break;
            }
            std::thread::sleep(Duration::from_millis(60));
        }

        self.height = self.content_height().unwrap_or(1200).max(100);
        Ok(())
    }

    fn content_height(&self) -> Result<u64> {
        let client = self.renderer.client();
        let mut client = client.lock().map_err(|e| anyhow!("lock poisoned: {e}"))?;
        let metrics = client.call(
            "Runtime.evaluate",
            serde_json::json!({
                "expression": "Math.min(Math.ceil(document.body.scrollHeight), 3600)",
                "returnByValue": true
            }),
        )?;
        Ok(metrics
            .get("result")
            .and_then(|r| r.get("value"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(1200))
    }

    pub fn height(&self) -> u64 {
        self.height
    }

    /// Drive a text selection at page coordinates. Chromium's synthetic mouse
    /// events don't reliably start a selection in headless mode, so we build
    /// the selection with DOM ranges from `caretRangeFromPoint` instead.
    /// `event_type` is mousePressed (start), mouseMoved (extend), or
    /// mouseReleased (finish).
    pub fn mouse(
        &self,
        event_type: &str,
        x: f64,
        y: f64,
        _button: &str,
        _buttons: i32,
        _click_count: i32,
    ) -> Result<()> {
        let expr = match event_type {
            "mousePressed" => format!(
                "(()=>{{var r=document.caretRangeFromPoint({x},{y});window.__selStart=r;\
                 if(r){{var s=getSelection();s.removeAllRanges();var c=document.createRange();\
                 c.setStart(r.startContainer,r.startOffset);c.setEnd(r.startContainer,r.startOffset);\
                 s.addRange(c);}}return !!r;}})()"
            ),
            "mouseMoved" | "mouseReleased" => format!(
                "(()=>{{var s=window.__selStart,e=document.caretRangeFromPoint({x},{y});\
                 if(!s||!e)return false;var r=document.createRange();\
                 r.setStart(s.startContainer,s.startOffset);r.setEnd(e.startContainer,e.startOffset);\
                 var sel=getSelection();sel.removeAllRanges();sel.addRange(r);return true;}})()"
            ),
            _ => return Ok(()),
        };
        let client = self.renderer.client();
        let mut client = client.lock().map_err(|e| anyhow!("lock poisoned: {e}"))?;
        client.call(
            "Runtime.evaluate",
            serde_json::json!({ "expression": expr, "returnByValue": true }),
        )?;
        Ok(())
    }

    /// Select all text in the document (used by Ctrl+A).
    pub fn select_all(&self) -> Result<()> {
        let client = self.renderer.client();
        let mut client = client.lock().map_err(|e| anyhow!("lock poisoned: {e}"))?;
        let resp = client.call(
            "Runtime.evaluate",
            serde_json::json!({
                "expression": "window.getSelection().selectAllChildren(document.body);window.getSelection().toString().length",
                "returnByValue": true
            }),
        )?;
        let _ = resp;
        Ok(())
    }

    /// Return the href of a link at the given page coordinates, if any.
    pub fn link_at(&self, x: f64, y: f64) -> Result<Option<String>> {
        let client = self.renderer.client();
        let mut client = client.lock().map_err(|e| anyhow!("lock poisoned: {e}"))?;
        let result = client.call(
            "Runtime.evaluate",
            serde_json::json!({
                "expression": format!(
                    "(()=>{{var e=document.elementFromPoint({x},{y});\
                     var a=e&&e.closest&&e.closest('a[href]');\
                     return a?a.href:null;}})()"
                ),
                "returnByValue": true
            }),
        )?;
        Ok(result
            .get("result")
            .and_then(|r| r.get("value"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_string))
    }

    /// Return the currently selected text, if any.
    pub fn selected_text(&self) -> Result<String> {
        let client = self.renderer.client();
        let mut client = client.lock().map_err(|e| anyhow!("lock poisoned: {e}"))?;
        let result = client.call(
            "Runtime.evaluate",
            serde_json::json!({
                "expression": "window.getSelection().toString()",
                "returnByValue": true
            }),
        )?;
        Ok(result
            .get("result")
            .and_then(|r| r.get("value"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string())
    }

    /// Clear the current selection.
    pub fn clear_selection(&self) -> Result<()> {
        let client = self.renderer.client();
        let mut client = client.lock().map_err(|e| anyhow!("lock poisoned: {e}"))?;
        client.call(
            "Runtime.evaluate",
            serde_json::json!({
                "expression": "window.getSelection().removeAllRanges(); window.__selStart=null;"
            }),
        )?;
        Ok(())
    }

    /// Capture the page (with any selection highlight) into a fresh JPEG path.
    pub fn capture(&mut self) -> Result<PathBuf> {
        self.seq += 1;
        let out = config::data_dir()
            .join("email-render")
            .join(format!("live-{}-{}.jpg", std::process::id(), self.seq));
        let client = self.renderer.client();
        let mut client = client.lock().map_err(|e| anyhow!("lock poisoned: {e}"))?;
        let result = client.call(
            "Page.captureScreenshot",
            serde_json::json!({
                "format": "jpeg",
                "quality": 92,
                "captureBeyondViewport": true,
                "clip": { "x": 0, "y": 0, "width": 720, "height": self.height, "scale": 1 }
            }),
        )?;
        let data = result
            .get("data")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow!("no screenshot data in CDP response"))?;
        let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, data)?;
        let mut file = std::fs::File::create(&out)?;
        file.write_all(&bytes)?;
        Ok(out)
    }
}

impl Drop for LiveEmailView {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.doc_path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn link_at_finds_anchor_href() {
        let mut view = LiveEmailView::start().expect("start live view");
        view.load(include_str!("fixtures/aliexpress_email.html"), None)
            .expect("load");
        let client = view.renderer.client();
        let mut c = client.lock().unwrap();
        let rect = c.call("Runtime.evaluate", serde_json::json!({
            "expression": "(()=>{const a=document.querySelector('a[href]');if(!a)return null;const r=a.getBoundingClientRect();return {x:r.left+r.width/2,y:r.top+r.height/2};})()",
            "returnByValue": true
        })).unwrap();
        drop(c);
        let Some(value) = rect.get("result").and_then(|r| r.get("value")) else {
            return; // fixture has no links
        };
        let href = view
            .link_at(value["x"].as_f64().unwrap_or(0.0), value["y"].as_f64().unwrap_or(0.0))
            .expect("link_at");
        assert!(
            href.as_deref().is_some_and(|h| h.starts_with("http")),
            "no href: {href:?}"
        );
    }

    #[test]
    #[ignore]
    fn select_text_via_cdp() {
        let html = include_str!("fixtures/aliexpress_email.html");
        let mut view = LiveEmailView::start().expect("start live view");
        view.load(html, None).expect("load");
        println!("page height: {}", view.height());

        // Find a text node's screen rect via JS, then drag across it.
        let client = view.renderer.client();
        let mut c = client.lock().unwrap();
        let rect = c.call("Runtime.evaluate", serde_json::json!({
            "expression": "(()=>{const w=document.createTreeWalker(document.body,NodeFilter.SHOW_TEXT);let n;while(n=w.nextNode()){if(n.textContent.trim().length>10){const r=document.createRange();r.selectNodeContents(n);const b=r.getBoundingClientRect();if(b.width>20&&b.top>=0&&b.top<2000)return {x:b.left+2,y:b.top+b.height/2,x2:b.right-2};}}return null;})()",
            "returnByValue": true
        })).unwrap();
        drop(c);
        println!("text rect: {rect}");
        let obj = rect.get("result").and_then(|r| r.get("value")).unwrap();
        let x1 = obj["x"].as_f64().unwrap_or(0.0);
        let y1 = obj["y"].as_f64().unwrap_or(0.0);
        let x2 = obj["x2"].as_f64().unwrap_or(0.0);
        view.mouse("mousePressed", x1, y1, "left", 1, 1).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
        view.mouse("mouseMoved", (x1+x2)/2.0, y1, "left", 1, 1).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
        view.mouse("mouseMoved", x2, y1, "left", 1, 1).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
        view.mouse("mouseReleased", x2, y1, "left", 0, 1).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
        let text = view.selected_text().unwrap();
        println!("selected via drag: {:?}", text);
        assert!(!text.trim().is_empty(), "drag selection returned no text");

        // capture should produce a real JPEG.
        let path = view.capture().unwrap();
        let size = std::fs::metadata(&path).unwrap().len();
        println!("capture: {size} bytes at {}", path.display());
        assert!(size > 5000, "capture too small");
    }
}

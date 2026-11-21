//! Fast email rendering via a persistent headless Chromium + CDP.
//! One browser process serves all emails; each render is just a navigate +
//! screenshot (tens of ms) instead of a full process launch.

use std::io::Write;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};

use super::cdp::CdpClient;
use crate::config;

const PORT: u16 = 9339;

pub struct FastRenderer {
    child: Child,
    client: Mutex<CdpClient>,
    target_id: String,
    profile: PathBuf,
}

impl FastRenderer {
    /// Launch a persistent Chromium and attach to a page target via CDP.
    pub fn start() -> Result<Self> {
        Self::start_with_port(PORT, "cdp-profile")
    }

    /// Launch Chromium on a specific debugging port and profile directory.
    pub fn start_with_port(port: u16, profile_name: &str) -> Result<Self> {
        let profile = config::data_dir().join(profile_name);
        std::fs::create_dir_all(&profile)?;
        let mut command = Command::new("chromium");
        // Put Chromium and all of its renderer/GPU children in their own
        // process group so Drop can terminate the whole tree. Killing only the
        // browser PID leaves orphaned renderers running (several GB of RAM).
        command.process_group(0);
        let child = command
            .args([
                "--headless=new",
                "--disable-gpu",
                "--no-sandbox",
                "--disable-dev-shm-usage",
                "--hide-scrollbars",
                "--disable-background-networking",
                "--disable-extensions",
                "--no-first-run",
                "--no-default-browser-check",
                "--log-level=3",
                "--remote-allow-origins=*",
                &format!("--remote-debugging-port={port}"),
                &format!("--user-data-dir={}", profile.display()),
                "--window-size=720,3600",
                "about:blank",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("launching persistent Chromium")?;

        // Wait for the DevTools endpoint.
        let client = reqwest::blocking::Client::new();
        let mut targets: Vec<serde_json::Value> = Vec::new();
        for _ in 0..40 {
            if let Ok(response) = client.get(format!("http://127.0.0.1:{port}/json/list")).send()
                && let Ok(list) = response.json::<Vec<serde_json::Value>>()
                    && !list.is_empty() {
                        targets = list;
                        break;
                    }
            std::thread::sleep(Duration::from_millis(100));
        }
        // The first target is often an injected extension background page.
        // Pick the real page target (about:blank) instead.
        let target = targets
            .iter()
            .find(|t| {
                t.get("type").and_then(serde_json::Value::as_str) == Some("page")
                    && t.get("url")
                        .and_then(serde_json::Value::as_str)
                        .map(|u| u.starts_with("about:") || u.starts_with("file://") || u == "chrome://newtab/")
                        .unwrap_or(false)
            })
            .or_else(|| targets.iter().find(|t| t.get("type").and_then(serde_json::Value::as_str) == Some("page")))
            .or_else(|| targets.first())
            .ok_or_else(|| anyhow!("Chromium DevTools target not available"))?;
        let ws_url = target
            .get("webSocketDebuggerUrl")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow!("no websocket URL in CDP target"))?;
        let target_id = target
            .get("id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        let client = CdpClient::connect(ws_url)?;
        Ok(Self {
            child,
            client: Mutex::new(client),
            target_id,
            profile,
        })
    }

    /// Render an HTML document to a PNG file via CDP. Fast path — no process spawn.
    pub fn render_to_png(&self, html: &str, output: &std::path::Path) -> Result<Duration> {
        let started = Instant::now();
        let mut client = self.client.lock().map_err(|e| anyhow!("lock poisoned: {e}"))?;

        // Write HTML to a temp file and navigate to it via file://. data: URLs
        // become megabytes for real emails and reliably fail to load.
        let tmp = config::data_dir()
            .join("email-render")
            .join(format!("cdp-{}.html", std::process::id()));
        {
            let mut file = std::fs::File::create(&tmp)?;
            file.write_all(html.as_bytes())?;
        }
        let navigate_url = format!("file://{}", tmp.display());
        client.call("Page.enable", serde_json::json!({}))?;
        client.call("Page.navigate", serde_json::json!({ "url": navigate_url }))?;

        // Data URIs load synchronously; a brief paint delay is enough.
        std::thread::sleep(Duration::from_millis(120));

        // Measure the document's real content height first, then capture only
        // that region — avoids a slow decode/re-encode crop step afterwards.
        let metrics = client.call("Runtime.evaluate", serde_json::json!({
            "expression": "Math.min(Math.ceil(document.body.scrollHeight), 2800)",
            "returnByValue": true
        }))?;
        let height = metrics
            .get("result")
            .and_then(|r| r.get("value"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(1200)
            .max(100) as u64;

        let result = client.call(
            "Page.captureScreenshot",
            serde_json::json!({
                "format": "jpeg",
                "quality": 92,
                "captureBeyondViewport": true,
                "clip": { "x": 0, "y": 0, "width": 720, "height": height, "scale": 1 }
            }),
        )?;
        let base64_data = result
            .get("data")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow!("no screenshot data in CDP response"))?;
        let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, base64_data)?;
        let mut file = std::fs::File::create(output)?;
        file.write_all(&bytes)?;
        let _ = std::fs::remove_file(&tmp);
        Ok(started.elapsed())
    }

    pub fn target_id(&self) -> &str {
        &self.target_id
    }

    /// Shared CDP client, for callers that drive the page directly (selection).
    pub fn client(&self) -> &Mutex<CdpClient> {
        &self.client
    }
}

impl Drop for FastRenderer {
    fn drop(&mut self) {
        // Negative PID targets the process group created with process_group(0).
        unsafe {
            libc::kill(-(self.child.id() as libc::pid_t), libc::SIGKILL);
        }
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.profile);
    }
}

fn urlencoding(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn benchmark_persistent_vs_spawn() {
        let html = include_str!("fixtures/marketing_email.html");
        let out = config::data_dir().join("email-render").join("bench-cdp.png");
        std::fs::create_dir_all(out.parent().unwrap()).unwrap();

        // Persistent browser (warm after first render).
        let renderer = FastRenderer::start().expect("start CDP chromium");
        let mut cdp_times = Vec::new();
        for i in 0..5 {
            let d = renderer.render_to_png(html, &out).expect("cdp render");
            if i > 0 { cdp_times.push(d.as_millis()); }
            println!("cdp render: {} ms", d.as_millis());
        }
        let cdp_avg = cdp_times.iter().sum::<u128>() / cdp_times.len() as u128;

        // Spawn-per-email (current approach).
        let out2 = config::data_dir().join("email-render").join("bench-spawn.png");
        let mut spawn_times = Vec::new();
        for _ in 0..3 {
            let _ = std::fs::remove_file(&out2);
            let start = Instant::now();
            let tmp = config::data_dir().join("email-render").join("bench.html");
            std::fs::write(&tmp, format!("<html><body>{html}</body></html>")).unwrap();
            let status = Command::new("chromium")
                .args(["--headless=new","--disable-gpu","--no-sandbox","--hide-scrollbars",
                       "--window-size=720,3600","--host-resolver-rules=MAP * 127.0.0.1"])
                .arg(format!("--screenshot={}", out2.display()))
                .arg(format!("file://{}", tmp.display()))
                .stdout(Stdio::null()).stderr(Stdio::null()).status().unwrap();
            assert!(status.success());
            spawn_times.push(start.elapsed().as_millis());
        }
        let spawn_avg = spawn_times.iter().sum::<u128>() / spawn_times.len() as u128;

        println!("CDP AVG {cdp_avg} ms  vs  SPAWN AVG {spawn_avg} ms  -> {:.1}x faster",
                 spawn_avg as f64 / cdp_avg.max(1) as f64);
        assert!(cdp_avg < spawn_avg / 2, "CDP not at least 2x faster");
    }
}

#[test]
#[ignore]
fn benchmark_warm_renders() {
    // Simulates the real app: one browser, many emails rendered sequentially.
    let html = include_str!("fixtures/marketing_email.html");
    let out = config::data_dir().join("email-render").join("bench-warm.png");
    std::fs::create_dir_all(out.parent().unwrap()).unwrap();
    let renderer = FastRenderer::start().expect("start CDP chromium");

    let start = Instant::now();
    let n = 8;
    for _ in 0..n {
        renderer.render_to_png(html, &out).expect("render");
    }
    let total = start.elapsed();
    let avg = total.as_millis() / n;
    println!("WARM CDP RENDER AVG {avg} ms over {n} emails (total {} ms)", total.as_millis());
    assert!(avg < 350, "warm render too slow: {avg} ms");
}


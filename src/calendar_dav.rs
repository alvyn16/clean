//! Minimal CalDAV client for synchronising local calendar events.
//!
//! Uses WebDAV `REPORT` with a `calendar-query` to pull VEVENTs, and `PUT` /
//! `DELETE` with ETags to push local changes back. No external calendar
//! library is needed: iCalendar's VEVENT format is simple enough to read and
//! write directly for the fields this app stores.

use anyhow::{anyhow, Context, Result};
use chrono::{NaiveDate, NaiveDateTime, Timelike};

#[derive(Debug, Clone)]
pub struct RemoteEvent {
    pub href: String,
    pub uid: String,
    pub etag: String,
    pub title: String,
    pub day: String,
    pub start_min: i32,
    pub end_min: i32,
}

pub struct CalDavClient {
    http: reqwest::blocking::Client,
    url: String,
    username: String,
    password: String,
}

impl CalDavClient {
    pub fn new(url: String, username: String, password: String) -> Result<Self> {
        let url = url.trim().trim_end_matches('/').to_string();
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(anyhow!("CalDAV URL must start with http:// or https://"));
        }
        Ok(Self {
            http: reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .danger_accept_invalid_certs(false)
                .build()?,
            url,
            username,
            password,
        })
    }

    /// Check the connection with a PROPFIND and return the calendar name.
    pub fn check(&self) -> Result<String> {
        let body = r#"<?xml version="1.0" encoding="utf-8"?>
<d:propfind xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav">
  <d:prop><d:displayname/><c:calendar-description/></d:prop>
</d:propfind>"#;
        let response = self
            .http
            .request(reqwest::Method::from_bytes(b"PROPFIND").unwrap(), &self.url)
            .basic_auth(&self.username, Some(&self.password))
            .header("Depth", "0")
            .header("Content-Type", "application/xml; charset=utf-8")
            .body(body)
            .send()
            .context("connecting to CalDAV server")?;
        let status = response.status();
        let text = response.text().unwrap_or_default();
        if !status.is_success() && status.as_u16() != 207 {
            return Err(anyhow!("CalDAV server returned {status}: {}", &text[..text.len().min(200)]));
        }
        let doc = roxmltree::Document::parse(&text).context("parsing CalDAV response")?;
        let name = find_text(&doc, "displayname").unwrap_or_else(|| "Calendar".into());
        Ok(name)
    }

    /// Fetch all VEVENTs in the calendar.
    pub fn fetch_events(&self) -> Result<Vec<RemoteEvent>> {
        let body = r#"<?xml version="1.0" encoding="utf-8"?>
<c:calendar-query xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav">
  <d:prop><d:getetag/><c:calendar-data/></d:prop>
  <c:filter>
    <c:comp-filter name="VCALENDAR">
      <c:comp-filter name="VEVENT"/>
    </c:comp-filter>
  </c:filter>
</c:calendar-query>"#;
        let response = self
            .http
            .request(reqwest::Method::from_bytes(b"REPORT").unwrap(), &self.url)
            .basic_auth(&self.username, Some(&self.password))
            .header("Depth", "1")
            .header("Content-Type", "application/xml; charset=utf-8")
            .body(body)
            .send()
            .context("querying CalDAV calendar")?;
        let status = response.status();
        let text = response.text().unwrap_or_default();
        if !status.is_success() && status.as_u16() != 207 {
            return Err(anyhow!("CalDAV REPORT failed ({status}): {}", &text[..text.len().min(200)]));
        }
        let doc = roxmltree::Document::parse(&text).context("parsing CalDAV REPORT response")?;

        let mut events = Vec::new();
        for response_node in doc.descendants().filter(|n| n.has_tag_name((DAV_NS, "response"))) {
            let href = find_text_node(response_node, "href").unwrap_or_default();
            let etag = find_text_node(response_node, "getetag")
                .map(|tag| tag.trim_matches('"').to_string())
                .unwrap_or_default();
            let data = response_node
                .descendants()
                .find(|n| n.has_tag_name((CALDAV_NS, "calendar-data")))
                .and_then(|n| n.text())
                .unwrap_or_default();
            if data.trim().is_empty() {
                continue;
            }
            if let Some(event) = parse_vevent(&href, &etag, data) {
                events.push(event);
            }
        }
        Ok(events)
    }

    /// Create or replace an event. Returns the new ETag.
    pub fn put_event(&self, event: &RemoteEvent, data: &str) -> Result<String> {
        let href = if event.href.trim().is_empty() {
            format!("{}/{}.ics", self.url, event.uid)
        } else if event.href.starts_with("http://") || event.href.starts_with("https://") {
            event.href.clone()
        } else {
            format!("{}/{}", self.url.trim_end_matches('/'), event.href.trim_start_matches('/'))
        };
        let mut request = self
            .http
            .put(&href)
            .basic_auth(&self.username, Some(&self.password))
            .header("Content-Type", "text/calendar; charset=utf-8")
            .body(data.to_string());
        if !event.etag.is_empty() {
            request = request.header("If-Match", format!("\"{}\"", event.etag.trim_matches('\"')));
        } else {
            request = request.header("If-None-Match", "*");
        }
        let response = request.send().context("uploading event to CalDAV")?;
        let status = response.status();
        if !status.is_success() {
            let text = response.text().unwrap_or_default();
            return Err(anyhow!("CalDAV PUT failed ({status}): {}", &text[..text.len().min(200)]));
        }
        let etag = response
            .headers()
            .get("etag")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .trim_matches('"')
            .to_string();
        Ok(etag)
    }

    pub fn delete_event(&self, event: &RemoteEvent) -> Result<()> {
        if event.href.trim().is_empty() {
            return Ok(());
        }
        let href = if event.href.starts_with("http://") || event.href.starts_with("https://") {
            event.href.clone()
        } else {
            format!("{}/{}", self.url.trim_end_matches('/'), event.href.trim_start_matches('/'))
        };
        let mut request = self
            .http
            .delete(&href)
            .basic_auth(&self.username, Some(&self.password));
        if !event.etag.is_empty() {
            request = request.header("If-Match", format!("\"{}\"", event.etag.trim_matches('\"')));
        }
        let response = request.send().context("deleting CalDAV event")?;
        if !response.status().is_success() && response.status().as_u16() != 404 {
            return Err(anyhow!("CalDAV DELETE failed ({})", response.status()));
        }
        Ok(())
    }
}

const DAV_NS: &str = "DAV:";
const CALDAV_NS: &str = "urn:ietf:params:xml:ns:caldav";

fn find_text<'a>(doc: &'a roxmltree::Document<'a>, tag: &str) -> Option<String> {
    doc.descendants()
        .find(|n| n.has_tag_name((DAV_NS, tag)) || n.tag_name().name() == tag)
        .and_then(|n| n.text())
        .map(str::to_string)
}

fn find_text_node<'a, 'b>(node: roxmltree::Node<'a, 'b>, tag: &str) -> Option<String> {
    node.descendants()
        .find(|n| n.has_tag_name((DAV_NS, tag)) || n.tag_name().name() == tag)
        .and_then(|n| n.text())
        .map(str::to_string)
}

/// Unfold RFC5545 folded lines (a line starting with a space continues the
/// previous one).
fn unfold_ical(data: &str) -> String {
    let mut out = String::new();
    for line in data.lines() {
        if line.starts_with([' ', '\t']) {
            out.push_str(line.trim_start());
        } else {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(line);
        }
    }
    out
}

fn parse_date_time(value: &str) -> Option<(String, i32)> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    // All-day event.
    if value.len() == 8 && !value.contains('T') {
        let date = NaiveDate::parse_from_str(value, "%Y%m%d").ok()?;
        return Some((date.format("%Y-%m-%d").to_string(), 0));
    }
    let local = value.trim_end_matches('Z');
    let naive = NaiveDateTime::parse_from_str(local, "%Y%m%dT%H%M%S").ok()?;
    Some((
        naive.format("%Y-%m-%d").to_string(),
        naive.hour() as i32 * 60 + naive.minute() as i32,
    ))
}

fn parse_vevent(href: &str, etag: &str, data: &str) -> Option<RemoteEvent> {
    let mut uid = String::new();
    let mut title = String::new();
    let mut start = None;
    let mut end = None;
    let mut in_event = false;

    for line in unfold_ical(data).lines() {
        if line == "BEGIN:VEVENT" {
            in_event = true;
            continue;
        }
        if line == "END:VEVENT" {
            break;
        }
        if !in_event {
            continue;
        }
        let (key, value) = match line.split_once(':') {
            Some(pair) => pair,
            None => continue,
        };
        let key = key.split(';').next().unwrap_or(key);
        match key {
            "UID" => uid = value.trim().to_string(),
            "SUMMARY" => title = unescape(value),
            "DTSTART" => start = parse_date_time(value),
            "DTEND" => end = parse_date_time(value),
            _ => {}
        }
    }
    if uid.is_empty() || start.is_none() {
        return None;
    }
    let (day, start_min) = start.unwrap();
    let end_min = match end {
        Some((end_day, end_min)) if end_day == day && end_min > start_min => end_min,
        _ => (start_min + 60).min(1440),
    };
    Some(RemoteEvent {
        href: href.to_string(),
        uid,
        etag: etag.to_string(),
        title,
        day,
        start_min,
        end_min,
    })
}

fn unescape(value: &str) -> String {
    value
        .replace("\\n", "\n")
        .replace("\\N", "\n")
        .replace("\\;", ";")
        .replace("\\,", ",")
        .replace("\\\\", "\\")
}

fn escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace(';', "\\;")
        .replace(',', "\\,")
        .replace('\n', "\\n")
}

/// Build a minimal iCalendar document for one event, using floating local
/// times that match the app's date/time pickers.
pub fn build_vevent(uid: &str, title: &str, day: &str, start_min: i32, end_min: i32) -> String {
    let date = NaiveDate::parse_from_str(day, "%Y-%m-%d").unwrap_or_else(|_| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap());
    let fmt = |minutes: i32| {
        let minutes = minutes.clamp(0, 1439);
        format!("{:02}{:02}00", minutes / 60, minutes % 60)
    };
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    format!(
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//clean//Calendar//EN\r\n\
         BEGIN:VEVENT\r\nUID:{uid}\r\nDTSTAMP:{stamp}\r\n\
         DTSTART:{}T{}\r\nDTEND:{}T{}\r\nSUMMARY:{}\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
        date.format("%Y%m%d"),
        fmt(start_min),
        date.format("%Y%m%d"),
        fmt(end_min.max(start_min + 1)),
        escape(title)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_vevent() {
        let data = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:abc@clean.local\r\n\
                    DTSTART:20261121T090000\r\nDTEND:20261121T103000\r\n\
                    SUMMARY:Team\\, meeting\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let event = parse_vevent("/cal/abc.ics", "etag1", data).unwrap();
        assert_eq!(event.uid, "abc@clean.local");
        assert_eq!(event.title, "Team, meeting");
        assert_eq!(event.day, "2026-11-21");
        assert_eq!(event.start_min, 540);
        assert_eq!(event.end_min, 630);
        assert_eq!(event.etag, "etag1");
    }

    #[test]
    fn builds_vevent() {
        let data = build_vevent("x@clean.local", "Coffee; now", "2026-11-21", 600, 630);
        assert!(data.contains("UID:x@clean.local"));
        assert!(data.contains("DTSTART:20261121T100000"));
        assert!(data.contains("SUMMARY:Coffee\\; now"));
        assert!(data.contains("\r\n"));
    }

    #[test]
    fn unfolds_lines() {
        assert_eq!(unfold_ical("SUMMARY:long\r\n  title"), "SUMMARY:longtitle");
    }
}

//! Two-way synchronisation between local events and a CalDAV calendar.

use std::collections::{HashMap, HashSet};

use anyhow::{anyhow, Result};
use tracing::{info, warn};

use crate::calendar_dav::{build_vevent, CalDavClient, RemoteEvent};
use crate::config::Config;
use crate::db::{Db, EventRow};
use crate::secrets;

#[derive(Debug, Clone, Copy, Default)]
pub struct SyncReport {
    pub pulled: usize,
    pub pushed: usize,
    pub deleted: usize,
}

pub fn is_configured(config: &Config) -> bool {
    !config.calendar.url.trim().is_empty()
}

pub fn password() -> Result<String> {
    secrets::get(secrets::CALDAV, "password")?
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow!("CalDAV password is missing; set it in Settings → Calendar"))
}

pub fn client_with(config: &Config, password: String) -> Result<CalDavClient> {
    let url = config.calendar.url.trim();
    if url.is_empty() {
        return Err(anyhow!("no CalDAV calendar configured"));
    }
    CalDavClient::new(url.to_string(), config.calendar.username.clone(), password)
}

/// Push local changes, then pull the server state and reconcile deletions.
pub fn sync(config: &Config, db: &Db) -> Result<SyncReport> {
    sync_with(config, db, password()?)
}

pub fn sync_with(config: &Config, db: &Db, password: String) -> Result<SyncReport> {
    let client = client_with(config, password)?;
    let mut report = SyncReport::default();
    let mut pushed_ids = HashSet::new();

    // 1. Remove locally deleted events from the server.
    for row in db.deleted_events()? {
        if row.remote {
            let remote = RemoteEvent {
                href: row.href.clone(),
                uid: row.uid.clone(),
                etag: row.etag.clone(),
                title: row.title.clone(),
                day: row.day.clone(),
                start_min: row.start_min,
                end_min: row.end_min,
            };
            if let Err(error) = client.delete_event(&remote) {
                warn!("could not delete CalDAV event {}: {error:#}", row.uid);
            }
        }
        db.hard_delete_event(row.id)?;
        report.deleted += 1;
    }

    // 2. Upload events created on this device.
    for row in db.pending_events()? {
        let uid = format!("{}@clean.local", uuid::Uuid::new_v4());
        let data = build_vevent(&uid, &row.title, &row.day, row.start_min, row.end_min);
        let remote = RemoteEvent {
            href: String::new(),
            uid: uid.clone(),
            etag: String::new(),
            title: row.title.clone(),
            day: row.day.clone(),
            start_min: row.start_min,
            end_min: row.end_min,
        };
        let etag = client.put_event(&remote, &data)?;
        let href = format!("{uid}.ics");
        db.mark_event_remote(row.id, &uid, &etag, &href)?;
        pushed_ids.insert(row.id);
        report.pushed += 1;
    }

    // 3. Pull current server state. Re-fetching here means uploads and deletes
    //    from step 1/2 are reflected without a second sync round.
    let remote_events = client.fetch_events()?;
    let remote_by_uid: HashMap<String, &RemoteEvent> = remote_events
        .iter()
        .map(|event| (event.uid.clone(), event))
        .collect();

    for remote in &remote_events {
        match db.event_by_uid(&remote.uid)? {
            Some(local) => {
                if local.etag != remote.etag
                    || local.title != remote.title
                    || local.day != remote.day
                    || local.start_min != remote.start_min
                    || local.end_min != remote.end_min
                {
                    db.update_remote_event(
                        local.id,
                        &remote.title,
                        &remote.day,
                        remote.start_min,
                        remote.end_min,
                        &remote.etag,
                        &remote.href,
                    )?;
                    report.pulled += 1;
                }
            }
            None => {
                db.add_event(&EventRow {
                    id: 0,
                    title: remote.title.clone(),
                    day: remote.day.clone(),
                    start_min: remote.start_min,
                    end_min: remote.end_min,
                    emoji: String::new(),
                    color: 0,
                    uid: remote.uid.clone(),
                    etag: remote.etag.clone(),
                    href: remote.href.clone(),
                    remote: true,
                    deleted: false,
                })?;
                report.pulled += 1;
            }
        }
    }

    // 4. Drop local copies of events deleted on the server, except ones we
    //    just uploaded (the server may not list them yet).
    for local in db.remote_events()? {
        if !pushed_ids.contains(&local.id) && !remote_by_uid.contains_key(&local.uid) {
            db.hard_delete_event(local.id)?;
            report.deleted += 1;
        }
    }

    info!(
        "calendar sync: {} pulled, {} pushed, {} deleted",
        report.pulled, report.pushed, report.deleted
    );
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    fn serve_one(listener: TcpListener, remote_ical: &'static str) -> std::thread::JoinHandle<()> {
        std::thread::spawn(move || {
            for _ in 0..8 {
                let Ok((mut stream, _)) = listener.accept() else { return };
                let mut buf = [0u8; 8192];
                let len = stream.read(&mut buf).unwrap_or(0);
                let request = String::from_utf8_lossy(&buf[..len]).to_string();
                let method = request.split_whitespace().next().unwrap_or("");
                if method == "PUT" {
                    let response = "HTTP/1.1 201 Created\r\nETag: \"new-etag\"\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                    let _ = stream.write_all(response.as_bytes());
                } else if method == "DELETE" {
                    let _ = stream.write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                } else {
                    let body = if method == "REPORT" {
                        format!(
                            "<?xml version=\"1.0\"?><d:multistatus xmlns:d=\"DAV:\" xmlns:c=\"urn:ietf:params:xml:ns:caldav\">\
                             <d:response><d:href>/cal/remote.ics</d:href><d:propstat><d:prop>\
                             <d:getetag>\"remote-etag\"</d:getetag><c:calendar-data>{}</c:calendar-data>\
                             </d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>\
                             </d:multistatus>",
                            remote_ical.replace('<', "&lt;").replace('>', "&gt;")
                        )
                    } else {
                        "<?xml version=\"1.0\"?><d:multistatus xmlns:d=\"DAV:\"><d:response>\
                         <d:propstat><d:prop><d:displayname>Test</d:displayname></d:prop>\
                         <d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>"
                            .to_string()
                    };
                    let response = format!(
                        "HTTP/1.1 207 Multi-Status\r\nContent-Type: application/xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(), body
                    );
                    let _ = stream.write_all(response.as_bytes());
                }
            }
        })
    }

    #[test]
    fn pushes_local_and_pulls_remote_events() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("test.db")).unwrap();
        db.add_event(&crate::db::EventRow {
            id: 0,
            title: "Local event".into(),
            day: "2026-11-22".into(),
            start_min: 540,
            end_min: 600,
            emoji: String::new(),
            color: 0,
            uid: String::new(),
            etag: String::new(),
            href: String::new(),
            remote: false,
            deleted: false,
        })
        .unwrap();

        let ical = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:remote@cal.local\r\n\
                    DTSTART:20261123T090000\r\nDTEND:20261123T100000\r\n\
                    SUMMARY:Remote event\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = serve_one(listener, ical);

        let config = Config {
            calendar: crate::config::CalendarConfig {
                url: format!("http://127.0.0.1:{port}/cal/"),
                username: "me".into(),
                sync_interval_secs: 300,
            },
            ..Config::default()
        };
        let report = sync_with(&config, &db, "secret".into()).unwrap();
        drop(server);

        assert_eq!(report.pushed, 1);
        assert_eq!(report.pulled, 1);
        let events = db.events().unwrap();
        assert_eq!(events.len(), 2);
        assert!(events.iter().any(|event| event.title == "Local event" && event.remote));
        assert!(events.iter().any(|event| event.title == "Remote event" && event.uid == "remote@cal.local"));
    }
}

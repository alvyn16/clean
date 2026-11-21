use std::path::Path;
use std::sync::Arc;

use chrono::{Datelike, Duration, Weekday};
use gpui_kit::*;

use crate::mail::html::{BlockKind, EmailBlock};
use crate::theme::palette::Palette;
use crate::ui::model::{label_color, ReaderMode, UiMessage};
use crate::ui::MessageAction;
use crate::ui::widgets::*;
use crate::ui::CleanApp;

pub fn reader(app: &CleanApp, cx: &mut Context<CleanApp>) -> impl IntoElement {
    let model = &app.model;
    let p = &model.palette;

    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w(px(0.0))
        .h_full()
        .bg(p.canvas.gpui())
        .when(model.reader != ReaderMode::Read, |this| {
            this.child(super::compose::compose(app, cx))
        })
        .when(model.reader == ReaderMode::Read, |this| {
            this.child(match model.current() {
                Some(message) => reading(app, message, cx).into_any_element(),
                None => empty(p).into_any_element(),
            })
        })
}

fn empty(p: &Palette) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .flex_1()
        .items_center()
        .justify_center()
        .gap(px(10.0))
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .size(px(46.0))
                .rounded_full()
                .bg(p.inset.with_alpha(0.6).gpui())
                .child(icon(IconName::MailOpen, 21.0, p.faint)),
        )
        .child(text(
            "Nothing selected",
            13.0,
            FontWeight::MEDIUM,
            p.muted,
        ))
        .child(text(
            "Pick a message to read it here.",
            11.5,
            FontWeight::NORMAL,
            p.faint,
        ))
}

fn reading(
    app: &CleanApp,
    message: &UiMessage,
    cx: &mut Context<CleanApp>,
) -> impl IntoElement {
    let model = &app.model;
    let p = &model.palette;
    let time = crate::ui::model::format_reader_time(message.row.date);
    let body = message
        .body
        .clone()
        .or_else(|| message.row.body_text.clone())
        .unwrap_or_else(|| message.preview());
    let _ = body;

    div()
        .flex()
        .flex_col()
        .size_full()
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .px(px(26.0))
                .pt(px(18.0))
                .pb(px(12.0))
                .child(
                    div()
                        .flex()
                        .items_start()
                        .gap(px(14.0))
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.0))
                                .font_family("Instrument Serif")
                                .text_size(px(25.0))
                                .line_height(px(30.0))
                                .text_color(p.text.gpui())
                                .child(message.row.subject.clone()),
                        )
                        .child(toolbar(app, cx)),
                )
                .when(!message.labels.is_empty(), |this| {
                    this.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(5.0))
                            .children(message.labels.iter().map(|label| {
                                let color = label_color(label, p.accent, p.is_dark);
                                chip(
                                    label.clone(),
                                    color,
                                    color.with_alpha(if p.is_dark { 0.14 } else { 0.10 }),
                                )
                            })),
                    )
                }),
        )
        .child(hairline(p.border.with_alpha(0.5)))
        .child(
            div()
                .id("reader-scroll")
                .flex()
                .flex_col()
                .flex_1()
                .overflow_y_scroll()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .px(px(26.0))
                        .pt(px(16.0))
                        .pb(px(20.0))
                        .max_w(px(760.0))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(10.0))
                                .child(sender_avatar(
                                    &message.row.from_addr,
                                    message.initial(),
                                    crate::ui::model::sender_color(message.sender(), p.is_dark),
                                    34.0,
                                    13.0,
                                    app.sender_avatar_for(&message.row.from_addr)
                                        .map(|p| gpui_kit::SharedString::from(p.to_string_lossy().into_owned())),
                                ))
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap(px(1.0))
                                        .child(
                                            div()
                                                .flex()
                                                .items_baseline()
                                                .gap(px(6.0))
                                                .child(text(
                                                    message.sender().to_string(),
                                                    12.5,
                                                    FontWeight::SEMIBOLD,
                                                    p.text,
                                                ))
                                                .child(text(
                                                    message.row.from_addr.clone(),
                                                    11.5,
                                                    FontWeight::NORMAL,
                                                    p.muted,
                                                )),
                                        )
                                        .child(text(
                                            format!("to {}", message.row.to_addrs),
                                            10.5,
                                            FontWeight::NORMAL,
                                            p.faint,
                                        )),
                                )
                                .child(div().flex_1())
                                .child(text(
                                    time,
                                    10.5,
                                    FontWeight::NORMAL,
                                    p.faint,
                                )),
                        )
                        .when(event_suggestion(message), |this| {
                            this.child(calendar_suggestion(p, message, cx))
                        })
                        .when(!app.attachments.is_empty(), |this| {
                            this.child(attachment_strip(app, cx))
                        })
                        .child(render_body(app, message, cx)),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .px(px(20.0))
                .py(px(11.0))
                .border_t(px(1.0))
                .border_color(p.border.with_alpha(0.45).gpui())
                .bg(p.surface.gpui())
                .child(
                    div()
                        .id("reader-reply")
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .h(px(38.0))
                        .px(px(16.0))
                        .flex_1()
                        .max_w(px(360.0))
                        .rounded_full()
                        .bg(p.inset.with_alpha(if p.is_dark { 0.9 } else { 1.0 }).gpui())
                        .border_1()
                        .border_color(p.border.with_alpha(0.5).gpui())
                        .cursor_pointer()
                        .hover(|style| style.bg(p.hover.gpui()))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.begin_reply(false, cx);
                            this.focus_field(crate::ui::Field::DraftBody, window, cx);
                        }))
                        .child(icon(IconName::CornerUpLeft, 14.0, p.muted))
                        .child(text(
                            format!("Reply to {}", short_name(message.sender())),
                            11.5,
                            FontWeight::MEDIUM,
                            p.muted,
                        )),
                )
                .child(div().flex_1())
                .child(
                    round_button("reader-reply-icon", IconName::ArrowUpRight, p).on_click(
                        cx.listener(|this, _, window, cx| {
                            this.begin_reply(false, cx);
                            this.focus_field(crate::ui::Field::DraftBody, window, cx);
                        }),
                    ),
                )
                .child(
                    round_button("reader-archive", IconName::Archive, p).on_click(cx.listener(
                        |this, _, _, cx| this.message_action(MessageAction::Archive, cx),
                    )),
                )
                .child(
                    round_button("reader-delete", IconName::Trash, p).on_click(cx.listener(
                        |this, _, _, cx| this.message_action(MessageAction::Trash, cx),
                    )),
                ),
        )
}

fn attachment_strip(app: &CleanApp, cx: &mut Context<CleanApp>) -> impl IntoElement {
    let p = &app.model.palette;
    div()
        .flex()
        .flex_col()
        .w_full()
        .gap(px(6.0))
        .px(px(20.0))
        .py(px(10.0))
        .border_b(px(1.0))
        .border_color(p.border.with_alpha(0.45).gpui())
        .child(text(
            format!(
                "{} attachment{}",
                app.attachments.len(),
                if app.attachments.len() == 1 { "" } else { "s" }
            ),
            10.5,
            FontWeight::SEMIBOLD,
            p.muted,
        ))
        .children(app.attachments.iter().enumerate().map(|(index, item)| {
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .h(px(30.0))
                .px(px(10.0))
                .rounded(px(7.0))
                .bg(p.inset.gpui())
                .child(icon(IconName::Paperclip, 13.0, p.accent))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .child(text(item.filename.clone(), 11.5, FontWeight::MEDIUM, p.text)),
                )
                .child(text(
                    crate::mail::attachments::human_size(item.size),
                    10.0,
                    FontWeight::NORMAL,
                    p.faint,
                ))
                .child(
                    div()
                        .id(("attachment-open", index))
                        .flex()
                        .items_center()
                        .h(px(22.0))
                        .px(px(8.0))
                        .rounded(px(5.0))
                        .cursor_pointer()
                        .hover(|style| style.bg(p.hover.gpui()))
                        .on_click(cx.listener(move |this, _, _, _| {
                            this.open_attachment(index);
                        }))
                        .child(text("Open", 10.5, FontWeight::MEDIUM, p.accent)),
                )
                .child(
                    div()
                        .id(("attachment-save", index))
                        .flex()
                        .items_center()
                        .h(px(22.0))
                        .px(px(8.0))
                        .rounded(px(5.0))
                        .cursor_pointer()
                        .hover(|style| style.bg(p.hover.gpui()))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.save_attachment(index, cx);
                        }))
                        .child(text("Save", 10.5, FontWeight::MEDIUM, p.accent)),
                )
        }))
}

fn toolbar(app: &CleanApp, cx: &mut Context<CleanApp>) -> impl IntoElement {
    let p = &app.model.palette;
    let action = |id: &'static str, icon_name: IconName| {
        icon_button(id, icon_name, p.muted, p.hover, 15.0)
    };
    div()
        .flex()
        .items_center()
        .gap(px(1.0))
        .child(
            action("reader-toolbar-reply", IconName::Reply)
                .on_click(cx.listener(|this, _, window, cx| {
                    this.begin_reply(false, cx);
                    this.focus_field(crate::ui::Field::DraftBody, window, cx);
                })),
        )
        .child(
            action("reader-toolbar-reply-all", IconName::ReplyAll)
                .on_click(cx.listener(|this, _, window, cx| {
                    this.begin_reply(true, cx);
                    this.focus_field(crate::ui::Field::DraftBody, window, cx);
                })),
        )
        .child(
            action("reader-toolbar-forward", IconName::Forward)
                .on_click(cx.listener(|this, _, window, cx| {
                    this.begin_forward(cx);
                    this.focus_field(crate::ui::Field::DraftTo, window, cx);
                })),
        )
        .child(
            action("reader-toolbar-archive", IconName::Archive)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.message_action(MessageAction::Archive, cx)
                })),
        )
        .child(
            action("reader-toolbar-trash", IconName::Trash)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.message_action(MessageAction::Trash, cx)
                })),
        )
        .child(
            action("reader-toolbar-open-browser", IconName::ExternalLink)
                .on_click(cx.listener(|this, _, _, _| {
                    tracing::info!("[browser-button] clicked");
                    let Some(message) = this.model.current() else {
                        return;
                    };
                    if let Some(html) = message.row.body_html.as_deref() {
                        let path = std::env::temp_dir().join(format!(
                            "clean-email-{}.html",
                            message.row.id
                        ));
                        // Write the same document the reader renders, with
                        // cached images rewritten to local files, so opening
                        // in a browser isn't full of broken images.
                        let document = crate::mail::render::build_document(html, None);
                        if std::fs::write(&path, document).is_ok() {
                            crate::mail::render::open_in_browser(
                                &path.to_string_lossy(),
                            );
                        }
                    }
                })),
        )
        .child(
            action("reader-toolbar-star", IconName::Heart)
                .on_click(cx.listener(|this, _, _, cx| this.star_current(cx))),
        )
        .child(
            action("reader-toolbar-unread", IconName::Mail)
                .on_click(cx.listener(|this, _, _, cx| {
                    let unread = this.model.current().map(|m| m.unread).unwrap_or(false);
                    this.message_action(MessageAction::MarkUnread(!unread), cx);
                })),
        )
        .child(action("reader-toolbar-more", IconName::Ellipsis))
}

fn short_name(name: &str) -> String {
    name.split_whitespace().next().unwrap_or(name).to_string()
}

fn event_suggestion(message: &UiMessage) -> bool {
    let haystack = format!(
        "{} {}",
        message.row.subject,
        message.body.clone().unwrap_or_default()
    )
    .to_ascii_lowercase();
    haystack.contains("saturday") || haystack.contains("party") || haystack.contains("meeting")
}

fn calendar_suggestion(
    p: &Palette,
    message: &UiMessage,
    cx: &mut Context<CleanApp>,
) -> impl IntoElement {
    let mut title = message.row.subject.clone();
    if let Some((head, _)) = title.split_once(':') {
        title = head.trim().to_string();
    }
    let title = title
        .trim_end_matches('?')
        .trim()
        .to_string();

    div()
        .mt(px(16.0))
        .flex()
        .items_center()
        .gap(px(10.0))
        .px(px(12.0))
        .py(px(10.0))
        .rounded(px(8.0))
        .bg(p.inset.with_alpha(if p.is_dark { 0.65 } else { 0.75 }).gpui())
        .border_1()
        .border_color(p.border.with_alpha(0.4).gpui())
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .size(px(28.0))
                .rounded(px(7.0))
                .bg(p.surface.gpui())
                .child(icon(IconName::CalendarDays, 14.0, p.accent)),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(1.0))
                .flex_1()
                .child(
                    div()
                        .text_size(px(11.5))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(p.text.gpui())
                        .child("Saturday · 2:00 PM"),
                )
                .child(
                    div()
                        .text_size(px(10.5))
                        .text_color(p.faint.gpui())
                        .child(format!("Found in this email · {}", title)),
                ),
        )
        .child(
            div()
                .id("suggestion-add")
                .flex()
                .items_center()
                .h(px(24.0))
                .px(px(10.0))
                .rounded(px(6.0))
                .bg(p.text.with_alpha(0.06).gpui())
                .text_size(px(11.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(p.text.gpui())
                .cursor_pointer()
                .hover(|style| style.bg(p.accent.with_alpha(0.16).gpui()))
                .on_click(cx.listener(|this, _, _, cx| {
                    let today = this.model.today;
                    let saturday = (1..=7)
                        .map(|days| today + Duration::days(days))
                        .find(|day| day.weekday() == Weekday::Sat)
                        .unwrap_or(today);
                    let title = this
                        .model
                        .current()
                        .map(|message| {
                            message
                                .row
                                .subject
                                .split(':')
                                .next()
                                .unwrap_or(&message.row.subject)
                                .trim()
                                .trim_end_matches('?')
                                .to_string()
                        })
                        .unwrap_or_else(|| "Event".into());
                    this.model.open_event_sheet(Some(title), saturday, 14 * 60, 15 * 60);
                    cx.notify();
                }))
                .child("Add"),
        )
        .child(
            icon_button("suggestion-close", IconName::X, p.faint, p.hover, 12.0),
        )
}

/// A shared cell holding the on-screen bounds of the rendered email image.
/// A `canvas` writes it on each layout so mouse positions can be converted to
/// page coordinates for the CDP selection.
type ImageBounds = Arc<std::sync::Mutex<Option<(f64, f64, f64, f64)>>>; // x, y, w, h

/// Convert a window-space mouse position into page coordinates using the
/// captured image bounds.
fn page_to_coords(pos: Point<Pixels>, bounds: &ImageBounds, page_height: f64) -> (f64, f64) {
    let b = bounds.lock().ok().and_then(|g| *g);
    let mx = pos.x.to_f64();
    let my = pos.y.to_f64();
    if let Some((x, y, w, h)) = b {
        let px = ((mx - x) / w * 720.0).max(0.0).min(720.0);
        let py = ((my - y) / h * page_height).max(0.0);
        (px, py)
    } else {
        (mx, my)
    }
}

/// Render an HTML email image with live mouse selection. Mouse drags are
/// forwarded into the CDP page; the resulting highlighted screenshot is shown.
fn live_email_image(
    _app: &CleanApp,
    path: &Path,
    message_id: i64,
    page_height: f64,
    cx: &mut Context<CleanApp>,
) -> AnyElement {
    let bounds: ImageBounds = Arc::new(std::sync::Mutex::new(None));

    let down_bounds = bounds.clone();
    let move_bounds = bounds.clone();
    let up_bounds = bounds.clone();
    let canvas_bounds = bounds.clone();

    div()
        .flex()
        .flex_col()
        .items_center()
        .pt(px(12.0))
        .w_full()
        .child(
            div()
                .relative()
                .child(
                    img(std::path::PathBuf::from(path.to_string_lossy().into_owned()))
                        .id(ElementId::Name(format!("rendered-email-{}", message_id).into()))
                        .w(px(720.0))
                        .flex_shrink_0()
                        .object_fit(gpui_kit::ObjectFit::Contain)
                        .rounded(px(4.0)),
                )
                // Capture the image bounds in window coordinates on every layout.
                .child(
                    canvas(
                        move |b, _, _| {
                            if let Ok(mut g) = canvas_bounds.lock() {
                                *g = Some((
                                    b.origin.x.to_f64(),
                                    b.origin.y.to_f64(),
                                    b.size.width.to_f64(),
                                    b.size.height.to_f64(),
                                ));
                            }
                        },
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .inset_0()
                    .w(px(720.0)),
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, event: &MouseDownEvent, _, _| {
                        let (x, y) = page_to_coords(event.position, &down_bounds, page_height);
                        if let Some(view) = this.live_view.clone() {
                            if let Ok(v) = view.lock() {
                                let _ = v.clear_selection();
                                let _ = v.mouse("mousePressed", x, y, "left", 1, 1);
                            }
                        }
                        this.live_dragging = true;
                    }),
                )
                .on_mouse_move(cx.listener(move |this, event: &MouseMoveEvent, _, _| {
                    if !this.live_dragging {
                        return;
                    }
                    let (x, y) = page_to_coords(event.position, &move_bounds, page_height);
                    if let Some(view) = this.live_view.clone() {
                        if let Ok(v) = view.lock() {
                            let _ = v.mouse("mouseMoved", x, y, "left", 1, 1);
                        }
                    }
                }))
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(move |this, event: &MouseUpEvent, _, cx| {
                        if !this.live_dragging {
                            return;
                        }
                        this.live_dragging = false;
                        let (x, y) = page_to_coords(event.position, &up_bounds, page_height);
                        if let Some(view) = this.live_view.clone() {
                            if let Ok(v) = view.lock() {
                                let _ = v.mouse("mouseReleased", x, y, "left", 0, 1);
                                // A click without a drag on a link opens it in
                                // the real browser, like a webmail client.
                                let no_selection = v
                                    .selected_text()
                                    .map(|text| text.trim().is_empty())
                                    .unwrap_or(false);
                                if no_selection {
                                    if let Ok(Some(href)) = v.link_at(x, y) {
                                        crate::mail::render::open_in_browser(&href);
                                    }
                                }
                            }
                        }
                        this.recapture_live(cx);
                    }),
                ),
        )
        .into_any_element()
}

/// Render an email body: prefer sanitized HTML blocks, fall back to plain text.
fn render_body(app: &CleanApp, message: &UiMessage, cx: &mut Context<CleanApp>) -> AnyElement {
    let p = &app.model.palette;
    let html_source = message.row.body_html.as_deref();

    // For HTML emails, prefer the live CDP capture (identical rendering, with
    // real selection) over the plain cached screenshot.
    if let Some(html) = html_source {
        if app.live_view_message == Some(message.row.id) {
            if let Some(path) = &app.live_capture_path {
                let page_height = app
                    .live_view
                    .as_ref()
                    .and_then(|v| v.lock().ok())
                    .map(|v| v.height() as f64)
                    .unwrap_or(2000.0);
                return live_email_image(app, path, message.row.id, page_height, cx);
            }
        }
        if let Some(path) = app.render_for_message(message.row.id, html) {
            return div()
                .flex()
                .flex_col()
                .items_center()
                .pt(px(12.0))
                .w_full()
                .child(
                    img(std::path::PathBuf::from(path.to_string_lossy().into_owned()))
                        .id(ElementId::Name(format!("rendered-email-{}", message.row.id).into()))
                        .w(px(720.0))
                        .flex_shrink_0()
                        .object_fit(gpui_kit::ObjectFit::Contain)
                        .rounded(px(4.0)),
                )
                .into_any_element();
        }
        // HTML email still rendering — calm placeholder, no flash of broken content.
        return div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(10.0))
            .h(px(220.0))
            .w_full()
            .child(icon(IconName::Loader, 22.0, p.faint))
            .child(text("Rendering email…", 12.0, FontWeight::NORMAL, p.faint))
            .into_any_element();
    }

    let container = div()
        .flex()
        .flex_col()
        .gap(px(10.0))
        .pt(px(18.0))
        .w_full();

    // Plain-text email: clean the text, split into paragraphs.
    let body = message
        .body
        .clone()
        .or_else(|| message.row.body_text.clone())
        .unwrap_or_else(|| message.preview());
    let lines: Vec<String> = body
        .lines()
        .map(|line| line.trim())
        .filter(|line| {
            !line.is_empty()
                && !line.starts_with("http://")
                && !line.starts_with("https://")
                && !line.starts_with("[[")
        })
        .map(str::to_string)
        .collect();
    container.children(lines.into_iter().map(|line| {
        div()
            .text_size(px(13.0))
            .line_height(px(20.5))
            .text_color(p.text.with_alpha(0.92).gpui())
            .child(line)
    }))
    .into_any_element()
}

fn render_block(app: &CleanApp, block: &EmailBlock) -> gpui_kit::AnyElement {
    let p = &app.model.palette;
    match block.kind {
        BlockKind::Divider => div()
            .h(px(1.0))
            .w_full()
            .my(px(4.0))
            .bg(p.border.with_alpha(0.5).gpui())
            .into_any_element(),
        BlockKind::Heading => div()
            .text_size(px(20.0))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(p.text.gpui())
            .line_height(px(26.0))
            .child(block.text.clone())
            .into_any_element(),
        BlockKind::Image => {
            let src = block.src.clone().unwrap_or_default();
            if let Some(path) = app.email_image_for(&src) {
                div()
                    .w_full()
                    .flex()
                    .justify_center()
                    .child(
                        img(std::path::PathBuf::from(path.to_string_lossy().into_owned()))
                            .id(ElementId::Name(format!("email-img-{}", src).into()))
                            .max_w(px(640.0))
                            .w_full()
                            .object_fit(gpui_kit::ObjectFit::Contain)
                            .rounded(px(6.0)),
                    )
                    .into_any_element()
            } else {
                div()
                    .w_full()
                    .py(px(8.0))
                    .text_size(px(11.0))
                    .text_color(p.faint.gpui())
                    .child(block.alt.clone().unwrap_or_else(|| "Image".into()))
                    .into_any_element()
            }
        }
        BlockKind::Video => {
            let src = block.src.clone().unwrap_or_default();
            div()
                .flex()
                .items_center()
                .gap(px(10.0))
                .p(px(12.0))
                .rounded(px(8.0))
                .bg(p.inset.with_alpha(if p.is_dark { 0.6 } else { 0.8 }).gpui())
                .border_1()
                .border_color(p.border.with_alpha(0.4).gpui())
                .child(icon(IconName::Video, 16.0, p.accent))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .child(text("Video", 12.0, FontWeight::SEMIBOLD, p.text))
                        .child(text(block.text.clone(), 10.5, FontWeight::NORMAL, p.muted)),
                )
                .child(
                    div()
                        .id(ElementId::Name(format!("video-open-{}", src).into()))
                        .flex()
                        .items_center()
                        .px(px(10.0))
                        .h(px(24.0))
                        .rounded(px(6.0))
                        .bg(p.accent.with_alpha(0.16).gpui())
                        .cursor_pointer()
                        .text_size(px(11.0))
                        .text_color(p.accent.gpui())
                        .on_click(move |_, _, _| {
                            crate::mail::render::open_in_browser(&src);
                        })
                        .child("Open video"),
                )
                .into_any_element()
        }
        BlockKind::Link => {
            let href = block.href.clone().unwrap_or_default();
            div()
                .text_size(px(13.0))
                .line_height(px(20.5))
                .text_color(p.accent.gpui())
                .cursor_pointer()
                .id(ElementId::Name(format!("email-link-{}", href).into()))
                .on_click(move |_, _, _| {
                    crate::mail::render::open_in_browser(&href);
                })
                .child(block.text.clone())
                .into_any_element()
        }
        BlockKind::Paragraph | BlockKind::Quote => {
            let is_quote = block.kind == BlockKind::Quote;
            div()
                .text_size(px(13.0))
                .line_height(px(20.5))
                .text_color(p.text.with_alpha(0.92).gpui())
                .when(is_quote, |this| {
                    this.border_l(px(2.0))
                        .border_color(p.accent.with_alpha(0.4).gpui())
                        .pl(px(10.0))
                })
                .child(block.text.clone())
                .into_any_element()
        }
    }
}


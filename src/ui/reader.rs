use chrono::{Datelike, Duration, Weekday};
use gpui_kit::*;

use crate::theme::palette::Palette;
use crate::ui::model::{label_color, paragraphs, ReaderMode, UiMessage};
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
    let blocks = paragraphs(&body);

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
                        .child(toolbar(p, cx)),
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
                                .child(avatar(
                                    message.initial(),
                                    crate::ui::model::sender_color(message.sender(), p.is_dark),
                                    34.0,
                                    13.0,
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
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(13.0))
                                .pt(px(18.0))
                                .children(blocks.iter().map(|paragraph| {
                                    div()
                                        .text_size(px(13.0))
                                        .line_height(px(20.5))
                                        .text_color(p.text.with_alpha(0.92).gpui())
                                        .child(paragraph.clone())
                                })),
                        ),
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
                            let Some(message) = this.model.current().cloned() else {
                                return;
                            };
                            this.model.draft_to = format!(
                                "{} <{}>",
                                message.sender(),
                                message.row.from_addr
                            );
                            this.model.draft_subject = format!("Re: {}", message.row.subject);
                            this.model.draft_body.clear();
                            this.model.reader = ReaderMode::Reply;
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
                            let Some(message) = this.model.current().cloned() else {
                                return;
                            };
                            this.model.draft_to =
                                format!("{} <{}>", message.sender(), message.row.from_addr);
                            this.model.draft_subject = format!("Re: {}", message.row.subject);
                            this.model.draft_body.clear();
                            this.model.reader = ReaderMode::Reply;
                            this.focus_field(crate::ui::Field::DraftBody, window, cx);
                        }),
                    ),
                )
                .child(
                    round_button("reader-archive", IconName::Archive, p).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.model.show_toast("Archived");
                            cx.notify();
                        },
                    )),
                )
                .child(
                    round_button("reader-delete", IconName::Trash, p).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.model.show_toast("Moved to trash");
                            cx.notify();
                        },
                    )),
                ),
        )
}

fn toolbar(p: &Palette, cx: &mut Context<CleanApp>) -> impl IntoElement {
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
                    let Some(message) = this.model.current().cloned() else {
                        return;
                    };
                    this.model.draft_to =
                        format!("{} <{}>", message.sender(), message.row.from_addr);
                    this.model.draft_subject = format!("Re: {}", message.row.subject);
                    this.model.draft_body.clear();
                    this.model.reader = ReaderMode::Reply;
                    this.focus_field(crate::ui::Field::DraftBody, window, cx);
                })),
        )
        .child(action("reader-toolbar-reply-all", IconName::ReplyAll))
        .child(action("reader-toolbar-forward", IconName::Forward))
        .child(action("reader-toolbar-archive", IconName::Archive))
        .child(action("reader-toolbar-trash", IconName::Trash))
        .child(action("reader-toolbar-print", IconName::Printer))
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

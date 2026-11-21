use gpui_kit::*;

use crate::theme::Rgba;
use crate::ui::model::{event_color, Field};
use crate::ui::widgets::*;
use crate::ui::CleanApp;

/// A more expressive, dog-day themed set for calendar events.
const EMOJIS: [&str; 6] = ["🐾", "🥳", "🐩", "🍦", "🎾", "😴"];

pub fn sheet(app: &CleanApp, cx: &mut Context<CleanApp>) -> impl IntoElement {
    let p = &app.model.palette;
    let draft = &app.model.event_draft;

    div()
        .absolute()
        .top(px(0.0))
        .left(px(0.0))
        .right(px(0.0))
        .bottom(px(0.0))
        .flex()
        .items_center()
        .justify_center()
        .bg(Rgba::hex(0x000000)
            .with_alpha(if p.is_dark { 0.45 } else { 0.22 })
            .gpui())
        .child(
            div()
                .flex()
                .flex_col()
                .w(px(440.0))
                .rounded(px(14.0))
                .bg(p.raised.gpui())
                .border_1()
                .border_color(p.border.with_alpha(0.8).gpui())
                .shadow_lg()
                .overflow_hidden()
                .child(header(app, cx))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(13.0))
                        .px(px(20.0))
                        .py(px(18.0))
                        .child(field(
                            app,
                            "event-title",
                            Field::EventTitle,
                            "EVENT TITLE",
                            &draft.title,
                            "What's happening?",
                            cx,
                        ))
                        .child(
                            div()
                                .flex()
                                .gap(px(10.0))
                                .child(field(
                                    app,
                                    "event-date",
                                    Field::EventDate,
                                    "DATE",
                                    &draft.day,
                                    "YYYY-MM-DD",
                                    cx,
                                ))
                                .child(field(
                                    app,
                                    "event-start",
                                    Field::EventStart,
                                    "STARTS",
                                    &draft.start,
                                    "09:00",
                                    cx,
                                ))
                                .child(field(
                                    app,
                                    "event-end",
                                    Field::EventEnd,
                                    "ENDS",
                                    &draft.end,
                                    "10:00",
                                    cx,
                                )),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(8.0))
                                .child(section_label("EMOJI", p.faint))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(7.0))
                                        .children(EMOJIS.iter().map(|emoji| {
                                            let selected = draft.emoji == *emoji;
                                            let emoji = *emoji;
                                            div()
                                                .id(ElementId::Name(
                                                    format!("event-emoji-{emoji}").into(),
                                                ))
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .size(px(30.0))
                                                .rounded(px(8.0))
                                                .bg(if selected {
                                                    p.accent.with_alpha(0.16).gpui()
                                                } else {
                                                    p.inset.gpui()
                                                })
                                                .border_1()
                                                .border_color(if selected {
                                                    p.accent.gpui()
                                                } else {
                                                    p.hover.with_alpha(0.0).gpui()
                                                })
                                                .cursor_pointer()
                                                .text_size(px(15.0))
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.model.event_draft.emoji = emoji.into();
                                                    cx.notify();
                                                }))
                                                .child(emoji)
                                        })),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(8.0))
                                .child(section_label("COLOR", p.faint))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(9.0))
                                        .children((0..6).map(|index| {
                                            let color = event_color(index, p.is_dark);
                                            let selected = draft.color == index;
                                            div()
                                                .id(("event-color", index))
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .size(px(22.0))
                                                .rounded_full()
                                                .cursor_pointer()
                                                .border_1()
                                                .border_color(if selected {
                                                    p.text.with_alpha(0.85).gpui()
                                                } else {
                                                    Rgba::new(0, 0, 0, 0).gpui()
                                                })
                                                .child(
                                                    div()
                                                        .size(px(15.0))
                                                        .rounded_full()
                                                        .bg(color.gpui()),
                                                )
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.model.event_draft.color = index;
                                                    cx.notify();
                                                }))
                                        })),
                                ),
                        ),
                )
                .child(actions(app, cx)),
        )
}

fn header(app: &CleanApp, cx: &mut Context<CleanApp>) -> impl IntoElement {
    let p = &app.model.palette;

    div()
        .flex()
        .items_center()
        .justify_between()
        .h(px(48.0))
        .px(px(16.0))
        .border_b(px(1.0))
        .border_color(p.border.with_alpha(0.6).gpui())
        .child(
            div()
                .font_family("Instrument Serif")
                .text_size(px(19.0))
                .text_color(p.text.gpui())
                .child("New event"),
        )
        .child(
            icon_button("event-close", IconName::X, p.muted, p.hover, 14.0).on_click(
                cx.listener(|this, _, _, cx| {
                    this.model.show_event_sheet = false;
                    this.model.focused = None;
                    cx.notify();
                }),
            ),
        )
}

fn field(
    app: &CleanApp,
    id: &'static str,
    target: Field,
    label: &'static str,
    value: &str,
    placeholder: &str,
    cx: &mut Context<CleanApp>,
) -> impl IntoElement {
    let p = &app.model.palette;
    let focused = app.model.focused == Some(target);

    div()
        .flex()
        .flex_col()
        .flex_1()
        .gap(px(6.0))
        .child(section_label(label, p.faint))
        .child(
            field_shell(p, focused)
                .id(id)
                .cursor_text()
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.focus_field(target, window, cx);
                }))
                .child(field_value(value, placeholder, focused, false, p)),
        )
}

fn actions(app: &CleanApp, cx: &mut Context<CleanApp>) -> impl IntoElement {
    let p = &app.model.palette;

    div()
        .flex()
        .items_center()
        .justify_end()
        .gap(px(8.0))
        .px(px(16.0))
        .py(px(12.0))
        .border_t(px(1.0))
        .border_color(p.border.with_alpha(0.5).gpui())
        .child(
            div()
                .id("event-cancel")
                .flex()
                .items_center()
                .h(px(32.0))
                .px(px(13.0))
                .rounded(px(8.0))
                .cursor_pointer()
                .text_size(px(11.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(p.muted.gpui())
                .hover(|style| style.bg(p.hover.gpui()))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.model.show_event_sheet = false;
                    this.model.focused = None;
                    cx.notify();
                }))
                .child("Cancel"),
        )
        .child(
            div()
                .id("event-create")
                .flex()
                .items_center()
                .h(px(32.0))
                .px(px(15.0))
                .rounded(px(8.0))
                .bg(p.accent.gpui())
                .text_size(px(11.5))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(p.accent_ink.gpui())
                .cursor_pointer()
                .shadow_sm()
                .hover(|style| style.opacity(0.92))
                .on_click(cx.listener(|this, _, _, cx| {
                    if this.model.add_event() {
                        this.toast("Event added to your calendar", cx);
                        this.sync_calendar(cx);
                    } else {
                        this.toast("Add a title first", cx);
                    }
                    cx.notify();
                }))
                .child("Add event"),
        )
}

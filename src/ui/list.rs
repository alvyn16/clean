use gpui_kit::*;

use crate::theme::palette::Palette;
use crate::ui::model::{format_list_time, label_color, sender_color, Field, Model};
use crate::ui::widgets::*;
use crate::ui::CleanApp;

pub fn message_list(
    app: &CleanApp,
    cx: &mut Context<CleanApp>,
    width: Pixels,
) -> impl IntoElement {
    let model = &app.model;
    let p = &model.palette;
    let visible: Vec<(usize, &crate::ui::model::UiMessage)> = model
        .messages
        .iter()
        .enumerate()
        .filter(|(_, message)| message.matches(&model.search))
        .collect();

    div()
        .flex()
        .flex_col()
        .w(width)
        .h_full()
        .flex_shrink_0()
        .bg(p.surface.gpui())
        .border_r(px(1.0))
        .border_color(p.border.with_alpha(0.55).gpui())
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .px(px(12.0))
                .pt(px(12.0))
                .pb(px(10.0))
                .child(search_field(model, p, cx))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .child(filter_pill(p, "Pinned 0", false))
                                .child(filter_pill(p, "All", true)),
                        )
                        .child(
                            icon_button(
                                "list-refresh",
                                IconName::RefreshCw,
                                p.muted,
                                p.hover,
                                13.0,
                            )
                            .on_click(cx.listener(|this, _, _, cx| this.sync_now(cx))),
                        ),
                ),
        )
        .child(div().px(px(0.0)).child(hairline(p.border.with_alpha(0.45))))
        .child(
            div()
                .id("message-scroll")
                .flex()
                .flex_col()
                .flex_1()
                .overflow_y_scroll()
                .when(visible.is_empty(), |this| {
                    this.child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .h(px(220.0))
                            .gap(px(8.0))
                            .child(icon(IconName::Inbox, 26.0, p.faint))
                            .child(text(
                                "Nothing here",
                                13.0,
                                FontWeight::MEDIUM,
                                p.muted,
                            ))
                            .child(text(
                                "Try a different folder or search.",
                                11.5,
                                FontWeight::NORMAL,
                                p.faint,
                            )),
                    )
                })
                .children(visible.iter().map(|(index, message)| {
                    let index = *index;
                    let selected = model.selected == Some(index);
                    message_row(app, p, message, selected)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.select_message(index, cx);
                            if matches!(
                                this.model.reader,
                                crate::ui::model::ReaderMode::Compose
                                    | crate::ui::model::ReaderMode::Reply
                            ) {
                                if let Some(field) = this.model.focused {
                                    this.focus_field(field, window, cx);
                                }
                            }
                        }))
                        .on_mouse_down(MouseButton::Right, cx.listener(move |this, _, _, cx| {
                            let is_draft = this
                                .model
                                .messages
                                .get(index)
                                .map(|message| message.draft_id.is_some())
                                .unwrap_or(false);
                            if is_draft {
                                return;
                            }
                            this.select_message(index, cx);
                            this.model.show_message_menu = true;
                            cx.notify();
                        }))
                }))
                .child(div().h(px(8.0))),
        )
}

fn search_field(model: &Model, p: &Palette, cx: &mut Context<CleanApp>) -> impl IntoElement {
    let focused = model.focused == Some(Field::Search);
    let value = model.search.clone();

    div()
        .id("search-field")
        .flex()
        .items_center()
        .gap(px(7.0))
        .h(px(32.0))
        .px(px(9.0))
        .rounded(px(8.0))
        .bg(p.inset.with_alpha(if p.is_dark { 0.85 } else { 1.0 }).gpui())
        .border_1()
        .border_color(if focused {
            p.accent.with_alpha(0.65).gpui()
        } else {
            p.border.with_alpha(0.5).gpui()
        })
        .cursor_text()
        .on_click(cx.listener(|this, _, window, cx| {
            this.focus_field(Field::Search, window, cx);
        }))
        .child(icon(IconName::Search, 13.0, p.faint))
        .child(
            div()
                .flex()
                .flex_1()
                .items_center()
                .overflow_hidden()
                .when(value.is_empty() && !focused, |this| {
                    this.child(text(
                        "Search all mail",
                        12.0,
                        FontWeight::NORMAL,
                        p.faint,
                    ))
                })
                .when(!value.is_empty(), |this| {
                    this.child(
                        div()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .text_size(px(12.0))
                            .text_color(p.text.gpui())
                            .child(value.clone()),
                    )
                })
                .when(focused, |this| {
                    this.child(
                        div()
                            .w(px(1.0))
                            .h(px(14.0))
                            .bg(p.accent.gpui()),
                    )
                })
                .when(value.is_empty() && focused, |this| {
                    this.child(text(
                        "Search all mail",
                        12.0,
                        FontWeight::NORMAL,
                        p.faint.with_alpha(0.6),
                    ))
                }),
        )
        .child(
            div()
                .px(px(4.0))
                .child(text("Ctrl K", 9.5, FontWeight::MEDIUM, p.faint)),
        )
}

fn filter_pill(p: &Palette, label: &str, active: bool) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .h(px(22.0))
        .px(px(9.0))
        .rounded_full()
        .bg(if active {
            p.text.with_alpha(0.07).gpui()
        } else {
            p.hover.with_alpha(0.0).gpui()
        })
        .text_size(px(11.0))
        .font_weight(if active {
            FontWeight::MEDIUM
        } else {
            FontWeight::NORMAL
        })
        .text_color(if active { p.text } else { p.muted }.gpui())
        .child(label.to_string())
}

fn message_row(
    app: &CleanApp,
    p: &Palette,
    message: &crate::ui::model::UiMessage,
    selected: bool,
) -> Stateful<Div> {
    let unread = message.unread;
    let avatar_color = sender_color(message.sender(), p.is_dark);
    let time = format_list_time(message.row.date, app.model.today);

    div()
        .id(("message", message.row.id as usize))
        .relative()
        .flex()
        .gap(px(9.0))
        .w_full()
        .px(px(12.0))
        .py(px(10.0))
        .cursor_pointer()
        .when(selected, |this| this.bg(p.selection.gpui()))
        .when(!selected, |this| {
            this.hover(|style| style.bg(p.hover.gpui()))
        })
        .border_b(px(1.0))
        .border_color(p.border.with_alpha(0.35).gpui())
        .when(selected, |this| {
            this.child(
                div()
                    .absolute()
                    .left(px(0.0))
                    .top(px(0.0))
                    .bottom(px(0.0))
                    .w(px(2.5))
                    .bg(p.accent.gpui()),
            )
        })
        .child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .pt(px(11.0))
                .w(px(6.0))
                .flex_shrink_0()
                .when(unread, |this| this.child(dot(p.accent, 6.0))),
        )
        .child(sender_avatar(
            &message.row.from_addr,
            message.initial(),
            avatar_color,
            30.0,
            12.0,
            app.sender_avatar_for(&message.row.from_addr)
                .map(|p| gpui_kit::SharedString::from(p.to_string_lossy().into_owned())),
        ))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w(px(0.0))
                .gap(px(2.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .flex_1()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .text_size(px(12.5))
                                .font_weight(if unread {
                                    FontWeight::SEMIBOLD
                                } else {
                                    FontWeight::MEDIUM
                                })
                                .text_color(p.text.gpui())
                                .child(message.sender().to_string()),
                        )
                        .child(
                            div()
                                .flex_shrink_0()
                                .text_size(px(10.0))
                                .font_weight(if unread {
                                    FontWeight::MEDIUM
                                } else {
                                    FontWeight::NORMAL
                                })
                                .text_color(if unread { p.accent } else { p.faint }.gpui())
                                .child(time),
                        ),
                )
                .child(
                    div()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_size(px(12.0))
                        .font_weight(if unread {
                            FontWeight::MEDIUM
                        } else {
                            FontWeight::NORMAL
                        })
                        .text_color(if selected { p.text } else { p.text }.gpui())
                        .child(message.row.subject.clone()),
                )
                .child(
                    div()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_size(px(11.5))
                        .text_color(p.muted.gpui())
                        .child(message.preview()),
                )
                .when(!message.labels.is_empty(), |this| {
                    this.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .pt(px(3.0))
                            .children(message.labels.iter().take(2).map(|label| {
                                let color = label_color(label, p.accent, p.is_dark);
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(4.0))
                                    .child(dot(color, 5.0))
                                    .child(
                                        div()
                                            .text_size(px(9.5))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(p.muted.gpui())
                                            .child(label.clone()),
                                    )
                            })),
                    )
                }),
        )
}

/// Context menu for the selected message: archive, trash, mark unread,
/// star/flag, and move to any known folder.
pub fn message_menu(app: &CleanApp, cx: &mut Context<CleanApp>) -> impl IntoElement {
    use crate::ui::MessageAction;

    let p = &app.model.palette;
    let current_unread = app.model.current().map(|m| m.unread).unwrap_or(false);
    let current_flagged = app
        .model
        .current()
        .map(|m| m.row.flags.contains("\\Flagged"))
        .unwrap_or(false);

    let item = |id: &'static str, label: &'static str| {
        div()
            .id(id)
            .flex()
            .items_center()
            .h(px(34.0))
            .px(px(10.0))
            .rounded(px(7.0))
            .cursor_pointer()
            .hover(|style| style.bg(p.hover.gpui()))
            .child(text(label, 12.0, FontWeight::NORMAL, p.text))
    };

    div()
        .absolute()
        .top(px(120.0))
        .left(px(260.0))
        .w(px(210.0))
        .p(px(6.0))
        .flex()
        .flex_col()
        .gap(px(1.0))
        .rounded(px(11.0))
        .bg(p.surface.gpui())
        .border_1()
        .border_color(p.border.with_alpha(0.9).gpui())
        .shadow_lg()
        .on_mouse_down(MouseButton::Left, cx.listener(|_, _, _, cx| cx.stop_propagation()))
        .child(
            item("msg-menu-unread", if current_unread { "Mark read" } else { "Mark unread" })
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.model.show_message_menu = false;
                    let unread = this.model.current().map(|m| m.unread).unwrap_or(false);
                    this.message_action(MessageAction::MarkUnread(!unread), cx);
                })),
        )
        .child(
            item("msg-menu-star", if current_flagged { "Remove star" } else { "Star" })
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.model.show_message_menu = false;
                    this.star_current(cx);
                })),
        )
        .child(
            item("msg-menu-archive", "Archive")
                .on_click(cx.listener(|this, _, _, cx| {
                    this.model.show_message_menu = false;
                    this.message_action(MessageAction::Archive, cx);
                })),
        )
        .child(
            item("msg-menu-trash", "Move to Trash")
                .on_click(cx.listener(|this, _, _, cx| {
                    this.model.show_message_menu = false;
                    this.message_action(MessageAction::Trash, cx);
                })),
        )
}

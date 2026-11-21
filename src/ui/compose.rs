use gpui_kit::*;

use crate::theme::Palette;
use crate::ui::model::{Field, ReaderMode};
use crate::ui::widgets::*;
use crate::ui::CleanApp;

pub fn compose(app: &CleanApp, cx: &mut Context<CleanApp>) -> impl IntoElement {
    let model = &app.model;
    let p = &model.palette;
    let reply = model.reader == ReaderMode::Reply;

    div()
        .flex()
        .flex_col()
        .size_full()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .h(px(46.0))
                .px(px(18.0))
                .border_b(px(1.0))
                .border_color(p.border.with_alpha(0.5).gpui())
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(9.0))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .size(px(24.0))
                                .rounded(px(7.0))
                                .bg(p.accent.with_alpha(0.14).gpui())
                                .child(icon(
                                    if reply {
                                        IconName::Reply
                                    } else {
                                        IconName::SquarePen
                                    },
                                    12.0,
                                    p.accent,
                                )),
                        )
                        .child(
                            div()
                                .text_size(px(12.5))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(p.text.gpui())
                                .child(if reply { "Reply" } else { "New message" }),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .when(model.draft_id.is_some(), |this| {
                            this.child(text(
                                "Draft saved",
                                10.5,
                                FontWeight::NORMAL,
                                p.faint,
                            ))
                        })
                        .child(
                            icon_button("compose-close", IconName::X, p.muted, p.hover, 14.0)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.close_composer(cx);
                                })),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(0.0))
                .px(px(18.0))
                .pt(px(6.0))
                .child(field_row(
                    app,
                    "compose-to",
                    Field::DraftTo,
                    "To",
                    &model.draft_to,
                    "name@example.com",
                    cx,
                ))
                .child(divider(p))
                .child(field_row(
                    app,
                    "compose-cc",
                    Field::DraftCc,
                    "Cc",
                    &model.draft_cc,
                    "name@example.com",
                    cx,
                ))
                .child(divider(p))
                .child(field_row(
                    app,
                    "compose-bcc",
                    Field::DraftBcc,
                    "Bcc",
                    &model.draft_bcc,
                    "name@example.com",
                    cx,
                ))
                .child(divider(p))
                .child(field_row(
                    app,
                    "compose-subject",
                    Field::DraftSubject,
                    "Subject",
                    &model.draft_subject,
                    "What's this about?",
                    cx,
                ))
                .child(divider(p))
                .child(field_row(
                    app,
                    "compose-attach-path",
                    Field::AttachmentPath,
                    "File",
                    &model.attachment_path,
                    "Paste a file path and press Enter to attach",
                    cx,
                ))
                .child(divider(p))
                .when(!app.compose_attachments.is_empty(), |this| {
                    this.child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(6.0))
                            .px(px(18.0))
                            .py(px(8.0))
                            .children(
                                app
                                    .compose_attachments
                                    .iter()
                                    .enumerate()
                                    .map(|(index, item)| {
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(6.0))
                                            .h(px(28.0))
                                            .px(px(9.0))
                                            .rounded(px(7.0))
                                            .bg(p.inset.gpui())
                                            .border_1()
                                            .border_color(p.border.with_alpha(0.5).gpui())
                                            .child(icon(IconName::Paperclip, 12.0, p.accent))
                                            .child(text(
                                                item.filename.clone(),
                                                11.0,
                                                FontWeight::MEDIUM,
                                                p.text,
                                            ))
                                            .child(text(
                                                crate::mail::attachments::human_size(item.size),
                                                10.0,
                                                FontWeight::NORMAL,
                                                p.faint,
                                            ))
                                            .child(
                                                div()
                                                    .id(("compose-attachment-remove", index))
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .size(px(17.0))
                                                    .rounded_full()
                                                    .cursor_pointer()
                                                    .hover(|style| style.bg(p.hover.gpui()))
                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                        this.remove_compose_attachment(index, cx);
                                                    }))
                                                    .child(icon(IconName::X, 10.0, p.muted)),
                                            )
                                    }),
                            ),
                    )
                }),
        )
        .child(
            div()
                .id("compose-body")
                .flex()
                .flex_col()
                .flex_1()
                .overflow_y_scroll()
                .px(px(18.0))
                .py(px(12.0))
                .cursor_text()
                .on_click(cx.listener(|this, _, window, cx| {
                    this.focus_field(Field::DraftBody, window, cx);
                }))
                .when(model.draft_body.is_empty(), |this| {
                    this.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(1.0))
                            .when(model.focused != Some(Field::DraftBody), |this| {
                                this.child(text(
                                    "Write your message…",
                                    13.0,
                                    FontWeight::NORMAL,
                                    p.faint,
                                ))
                            })
                            .when(model.focused == Some(Field::DraftBody), |this| {
                                this.child(div().w(px(1.5)).h(px(17.0)).bg(p.accent.gpui()))
                            }),
                    )
                })
                .when(!model.draft_body.is_empty(), |this| {
                    this.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(9.0))
                            .children(model.draft_body.split('\n').map(|line| {
                                div()
                                    .flex()
                                    .items_center()
                                    .min_h(px(19.0))
                                    .text_size(px(13.0))
                                    .line_height(px(20.0))
                                    .text_color(p.text.with_alpha(0.92).gpui())
                                    .child(if line.is_empty() {
                                        " ".to_string()
                                    } else {
                                        line.to_string()
                                    })
                            }))
                            .when(model.focused == Some(Field::DraftBody), |this| {
                                this.child(div().w(px(1.5)).h(px(16.0)).bg(p.accent.gpui()))
                            }),
                    )
                }),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.0))
                .px(px(16.0))
                .py(px(10.0))
                .border_t(px(1.0))
                .border_color(p.border.with_alpha(0.5).gpui())
                .bg(p.surface.gpui())
                .child(
                    icon_button("compose-attach", IconName::Paperclip, p.muted, p.hover, 14.0)
                        .on_click(cx.listener(|_, _, _, cx| {
                            let receiver = cx.prompt_for_paths(PathPromptOptions {
                                files: true,
                                directories: false,
                                multiple: true,
                                prompt: Some("Attach files".into()),
                            });
                            cx.spawn(async move |this, cx| {
                                match receiver.await {
                                    Ok(Ok(Some(paths))) => {
                                        this.update(cx, |this, cx| {
                                            this.attach_files(paths, cx);
                                            cx.notify();
                                        })
                                        .ok();
                                    }
                                    Ok(Ok(None)) | Err(_) => {}
                                    Ok(Err(error)) => {
                                        this.update(cx, |this, cx| {
                                            this.toast(
                                                format!("Could not open file picker: {error:#}"),
                                                cx,
                                            );
                                        })
                                        .ok();
                                    }
                                }
                            })
                            .detach();
                        })),
                )
                .child(compose_tool("compose-image", IconName::Image, p, cx))
                .child(compose_tool("compose-link", IconName::AtSign, p, cx))
                .child(compose_tool("compose-calendar", IconName::CalendarPlus, p, cx))
                .child(div().flex_1())
                .child(
                    div()
                        .id("compose-discard")
                        .flex()
                        .items_center()
                        .h(px(30.0))
                        .px(px(11.0))
                        .rounded(px(7.0))
                        .cursor_pointer()
                        .text_size(px(11.5))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(p.muted.gpui())
                        .hover(|style| style.bg(p.hover.gpui()))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.discard_draft(cx);
                        }))
                        .child("Discard"),
                )
                .child(
                    div()
                        .id("compose-send")
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .h(px(30.0))
                        .px(px(14.0))
                        .rounded(px(7.0))
                        .bg(p.accent.gpui())
                        .text_size(px(11.5))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(p.accent_ink.gpui())
                        .cursor_pointer()
                        .shadow_sm()
                        .hover(|style| style.opacity(0.92))
                        .on_click(cx.listener(|this, _, _, cx| this.send_draft(cx)))
                        .child("Send")
                        .child(icon(IconName::Send, 12.0, p.accent_ink)),
                ),
        )
}

fn field_row(
    app: &CleanApp,
    id: &'static str,
    field: Field,
    label: &'static str,
    value: &str,
    placeholder: &str,
    cx: &mut Context<CleanApp>,
) -> impl IntoElement {
    let p = &app.model.palette;
    let focused = app.model.focused == Some(field);

    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(10.0))
        .h(px(42.0))
        .cursor_text()
        .on_click(cx.listener(move |this, _, window, cx| {
            this.focus_field(field, window, cx);
        }))
        .child(
            div()
                .w(px(52.0))
                .flex_shrink_0()
                .text_size(px(11.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(p.faint.gpui())
                .child(label),
        )
        .child(field_value(value, placeholder, focused, false, p))
}

fn divider(p: &Palette) -> impl IntoElement {
    div()
        .h(px(1.0))
        .w_full()
        .bg(p.border.with_alpha(0.4).gpui())
}

fn compose_tool(
    id: &'static str,
    icon_name: IconName,
    p: &Palette,
    cx: &mut Context<CleanApp>,
) -> impl IntoElement {
    icon_button(id, icon_name, p.muted, p.hover, 14.0).on_click(cx.listener(
        |this, _, _, cx| {
            this.toast("Attachments are coming soon", cx);
        },
    ))
}

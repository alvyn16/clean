use gpui_kit::*;

use crate::theme::palette::{Palette, Rgba};
use crate::ui::model::{MailView, Model};
use crate::ui::widgets::*;
use crate::ui::CleanApp;

pub fn topbar_bg(palette: &Palette) -> Rgba {
    match palette.mode {
        crate::theme::ThemeMode::White => palette.surface,
        crate::theme::ThemeMode::Black => Rgba::hex(0x0D0E11),
        crate::theme::ThemeMode::Omarchy => palette.sidebar.mix(palette.accent, 0.10),
    }
}

pub fn topbar(app: &CleanApp, cx: &mut Context<CleanApp>) -> impl IntoElement {
    let model: &Model = &app.model;
    let p = &model.palette;
    let bg = topbar_bg(p);
    let selected = model.view;

    div()
        .flex()
        .items_center()
        .justify_between()
        .h(px(52.0))
        .px(px(16.0))
        .pl(px(22.0))
        .bg(bg.gpui())
        .border_b(px(1.0))
        .border_color(p.border.with_alpha(0.55).gpui())
        .child(
            serif(30.0, FontWeight::NORMAL, p.text)
                .line_height(px(34.0))
                .id("topbar-wordmark")
                .on_mouse_down(MouseButton::Left, |_, window, _| {
                    window.start_window_move();
                })
                .child("clean"),
        )
        .child(drag_spacer("topbar-drag-left"))
        .child(view_switch(p, selected, cx))
        .child(drag_spacer("topbar-drag-right"))
        .child(
            div()
                .id("topbar-actions")
                .on_mouse_down(MouseButton::Left, cx.listener(|_, _, _, cx| cx.stop_propagation()))
                .flex()
                .items_center()
                .gap(px(6.0))
                .when(model.config.accounts.len() > 0, |this| {
                    this.child(
                        icon_button(
                            "topbar-sync",
                            IconName::RefreshCw,
                            if model.syncing { p.accent } else { p.muted },
                            p.hover,
                            15.0,
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.sync_now(cx))),
                    )
                })
                .child(
                    div()
                        .id("topbar-compose")
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(px(30.0))
                        .rounded_full()
                        .bg(p.accent.gpui())
                        .cursor_pointer()
                        .shadow_sm()
                        .hover(|style| style.opacity(0.9))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.model.reader = crate::ui::model::ReaderMode::Compose;
                            this.model.focused = Some(crate::ui::Field::DraftTo);
                            cx.notify();
                        }))
                        .child(icon(IconName::SquarePen, 14.0, p.accent_ink)),
                ),
        )
}

fn drag_spacer(id: &'static str) -> impl IntoElement {
    div()
        .id(id)
        .flex_1()
        .h_full()
        .on_mouse_down(MouseButton::Left, |_, window, _| {
            window.start_window_move();
        })
}

fn view_switch(p: &Palette, selected: MailView, cx: &mut Context<CleanApp>) -> impl IntoElement {
    let offset = match selected {
        MailView::Mail => 2.0,
        MailView::Calendar => 96.0,
    };

    div()
        .id("view-switch")
        .on_mouse_down(MouseButton::Left, cx.listener(|_, _, _, cx| cx.stop_propagation()))
        .relative()
        .flex()
        .items_center()
        .w(px(190.0))
        .h(px(34.0))
        .rounded_full()
        .bg(p.inset.with_alpha(if p.is_dark { 0.9 } else { 1.0 }).gpui())
        .border_1()
        .border_color(p.border.with_alpha(0.6).gpui())
        .child(
            div()
                .absolute()
                .top(px(3.0))
                .w(px(92.0))
                .h(px(26.0))
                .rounded_full()
                .bg(p.surface.gpui())
                .shadow_sm()
                .left(px(offset)),
        )
        .child(
            div()
                .flex()
                .w_full()
                .h_full()
                .child(segment(
                    "segment-mail",
                    IconName::Mail,
                    "Mail",
                    selected == MailView::Mail,
                    p,
                    cx,
                ))
                .child(segment(
                    "segment-calendar",
                    IconName::CalendarDays,
                    "Calendar",
                    selected == MailView::Calendar,
                    p,
                    cx,
                )),
        )
}

fn segment(
    id: &'static str,
    icon_name: IconName,
    label: &'static str,
    active: bool,
    p: &Palette,
    cx: &mut Context<CleanApp>,
) -> impl IntoElement {
    let color = if active { p.text } else { p.muted };
    let view = if label == "Mail" {
        MailView::Mail
    } else {
        MailView::Calendar
    };

    div()
        .id(id)
        .flex()
        .flex_1()
        .h_full()
        .items_center()
        .justify_center()
        .gap(px(6.0))
        .rounded_full()
        .cursor_pointer()
        .text_size(px(12.0))
        .font_weight(if active {
            FontWeight::MEDIUM
        } else {
            FontWeight::NORMAL
        })
        .text_color(color.gpui())
        .on_click(cx.listener(move |this, _, _, cx| {
            this.model.view = view;
            cx.notify();
        }))
        .child(icon(icon_name, 13.0, color))
        .child(label)
}

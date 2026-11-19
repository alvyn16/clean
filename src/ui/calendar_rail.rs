use chrono::{Datelike, Duration, NaiveDate};
use gpui_kit::*;

use crate::theme::Palette;
use crate::ui::model::{event_color, Model};
use crate::ui::widgets::*;
use crate::ui::CleanApp;

pub fn rail(app: &CleanApp, cx: &mut Context<CleanApp>) -> impl IntoElement {
    let model = &app.model;
    let p = &model.palette;
    let upcoming = model.upcoming();

    div()
        .flex()
        .flex_col()
        .w(px(252.0))
        .h_full()
        .flex_shrink_0()
        .bg(p.surface.gpui())
        .border_l(px(1.0))
        .border_color(p.border.with_alpha(0.55).gpui())
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .px(px(16.0))
                .pt(px(14.0))
                .pb(px(10.0))
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(p.text.gpui())
                        .child("Upcoming"),
                )
                .child(
                    div()
                        .id("rail-calendar")
                        .flex()
                        .items_center()
                        .gap(px(4.0))
                        .px(px(6.0))
                        .h(px(22.0))
                        .rounded(px(6.0))
                        .cursor_pointer()
                        .text_size(px(10.5))
                        .text_color(p.faint.gpui())
                        .hover(|style| style.bg(p.hover.gpui()))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.model.view = crate::ui::MailView::Calendar;
                            cx.notify();
                        }))
                        .child(
                            div()
                                .text_size(px(10.5))
                                .font_weight(FontWeight::MEDIUM)
                                .child(month_label(model.today)),
                        )
                        .child(icon(IconName::ChevronRight, 11.0, p.faint)),
                ),
        )
        .child(month_grid(model, p))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .px(px(16.0))
                .pt(px(14.0))
                .pb(px(7.0))
                .child(section_label("NEXT UP", p.faint))
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(p.faint.gpui())
                        .child(format!("{} events", upcoming.len())),
                ),
        )
        .child(
            div()
                .id("rail-scroll")
                .flex()
                .flex_col()
                .gap(px(6.0))
                .flex_1()
                .overflow_y_scroll()
                .px(px(12.0))
                .pb(px(14.0))
                .when(upcoming.is_empty(), |this| {
                    this.child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap(px(7.0))
                            .py(px(28.0))
                            .child(icon(IconName::CalendarDays, 20.0, p.faint))
                            .child(text(
                                "No events ahead",
                                11.5,
                                FontWeight::NORMAL,
                                p.faint,
                            )),
                    )
                })
                .children(upcoming.into_iter().map(|event| {
                    let color = event_color(event.color, p.is_dark);
                    let day = model.day_label(event.day);
                    div()
                        .id(("rail-event", event.id))
                        .flex()
                        .gap(px(9.0))
                        .p(px(9.0))
                        .rounded(px(9.0))
                        .bg(p.inset.with_alpha(if p.is_dark { 0.55 } else { 0.8 }).gpui())
                        .border_1()
                        .border_color(p.border.with_alpha(0.35).gpui())
                        .cursor_pointer()
                        .hover(|style| style.border_color(p.border.gpui()))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .size(px(30.0))
                                .rounded(px(8.0))
                                .bg(color.with_alpha(if p.is_dark { 0.18 } else { 0.12 }).gpui())
                                .text_size(px(14.0))
                                .child(event.emoji.clone()),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(2.0))
                                .flex_1()
                                .min_w(px(0.0))
                                .child(
                                    div()
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .text_ellipsis()
                                        .text_size(px(11.5))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(p.text.gpui())
                                        .child(event.title.clone()),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(5.0))
                                        .child(dot(color, 5.0))
                                        .child(
                                            div()
                                                .text_size(px(10.0))
                                                .text_color(p.muted.gpui())
                                                .child(format!("{day} · {}", event.time_label())),
                                        ),
                                ),
                        )
                })),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .px(px(16.0))
                .py(px(10.0))
                .border_t(px(1.0))
                .border_color(p.border.with_alpha(0.45).gpui())
                .child(text(
                    "Calendar",
                    11.0,
                    FontWeight::MEDIUM,
                    p.muted,
                ))
                .child(
                    div()
                        .id("rail-open-calendar")
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(px(22.0))
                        .rounded(px(6.0))
                        .cursor_pointer()
                        .hover(|style| style.bg(p.hover.gpui()))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.model.view = crate::ui::MailView::Calendar;
                            cx.notify();
                        }))
                        .child(icon(IconName::ArrowUpRight, 12.0, p.muted)),
                ),
        )
}

pub fn month_label(day: NaiveDate) -> String {
    day.format("%B").to_string()
}

fn month_grid(model: &Model, p: &Palette) -> impl IntoElement {
    let today = model.today;
    let first = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap_or(today);
    let offset = first.weekday().num_days_from_sunday() as i64;
    let start = first - Duration::days(offset);
    let cells: Vec<NaiveDate> = (0..35).map(|index| start + Duration::days(index)).collect();

    div()
        .flex()
        .flex_col()
        .gap(px(3.0))
        .px(px(12.0))
        .child(
            div()
                .flex()
                .gap(px(2.0))
                .children(["S", "M", "T", "W", "T", "F", "S"].iter().map(|label| {
                    div()
                        .flex()
                        .flex_1()
                        .justify_center()
                        .text_size(px(9.5))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(p.faint.gpui())
                        .child(*label)
                })),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .children(cells.chunks(7).map(|week| {
                    div()
                        .flex()
                        .gap(px(2.0))
                        .children(week.iter().map(|day| {
                            let current_month = day.month() == today.month();
                            let is_today = *day == today;
                            let has_event = model.events.iter().any(|event| event.day == *day);
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .justify_center()
                                .gap(px(2.0))
                                .flex_1()
                                .h(px(24.0))
                                .rounded(px(6.0))
                                .when(is_today, |this| this.bg(p.accent.gpui()))
                                .child(
                                    div()
                                        .text_size(px(10.0))
                                        .font_weight(if is_today {
                                            FontWeight::SEMIBOLD
                                        } else {
                                            FontWeight::NORMAL
                                        })
                                        .text_color(
                                            (if is_today {
                                                p.accent_ink
                                            } else if current_month {
                                                p.muted
                                            } else {
                                                p.faint.with_alpha(0.55)
                                            })
                                            .gpui(),
                                        )
                                        .child(day.day().to_string()),
                                )
                                .when(has_event && !is_today, |this| {
                                    this.child(dot(p.accent.with_alpha(0.8), 3.0))
                                })
                        }))
                })),
        )
}

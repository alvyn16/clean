use chrono::{Datelike, Duration};
use gpui_kit::*;

use crate::theme::Palette;
use crate::ui::model::{event_color, format_clock, now_minutes, week_start, CalendarEvent, Model};
use crate::ui::widgets::*;
use crate::ui::CleanApp;

const START_HOUR: u32 = 7;
const END_HOUR: u32 = 21;
const HOUR_HEIGHT: f32 = 54.0;
const GUTTER: f32 = 58.0;

pub fn calendar_view(app: &CleanApp, cx: &mut Context<CleanApp>) -> impl IntoElement {
    let model = &app.model;
    let p = &model.palette;
    let days: Vec<_> = (0..7)
        .map(|index| model.week_start + Duration::days(index))
        .collect();
    let mut columns = Vec::new();
    for (index, day) in days.iter().enumerate() {
        columns.push(day_column(app, model, p, *day, index == 6, cx));
    }

    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w(px(0.0))
        .h_full()
        .bg(p.canvas.gpui())
        .child(toolbar(app, cx))
        .child(week_header(app, &days))
        .child(
            div()
                .flex()
                .flex_1()
                .min_h(px(0.0))
                .px(px(18.0))
                .pb(px(16.0))
                .child(
                    div()
                        .id("calendar-scroll")
                        .flex()
                        .flex_1()
                        .min_h(px(0.0))
                        .overflow_y_scroll()
                        .rounded(px(12.0))
                        .border_1()
                        .border_color(p.border.with_alpha(0.55).gpui())
                        .bg(p.surface.gpui())
                        .child(time_gutter(p))
                        .child(
                            div()
                                .flex()
                                .flex_1()
                                .min_w(px(0.0))
                                .children(columns),
                        ),
                ),
        )
}

fn toolbar(app: &CleanApp, cx: &mut Context<CleanApp>) -> impl IntoElement {
    let model = &app.model;
    let p = &model.palette;
    let start = model.week_start;
    let end = start + Duration::days(6);
    let title = if start.month() == end.month() {
        format!("{} {}", start.format("%B"), start.year())
    } else if start.year() == end.year() {
        format!(
            "{} – {} {}",
            start.format("%b"),
            end.format("%b"),
            start.year()
        )
    } else {
        format!(
            "{} {} – {} {}",
            start.format("%b"),
            start.year(),
            end.format("%b"),
            end.year()
        )
    };

    div()
        .flex()
        .items_center()
        .justify_between()
        .h(px(58.0))
        .px(px(20.0))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(12.0))
                .child(
                    div()
                        .font_family("Instrument Serif")
                        .text_size(px(23.0))
                        .text_color(p.text.gpui())
                        .child(title),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(2.0))
                        .child(
                            icon_button(
                                "cal-prev",
                                IconName::ChevronLeft,
                                p.muted,
                                p.hover,
                                14.0,
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.model.week_start = this.model.week_start - Duration::days(7);
                                cx.notify();
                            })),
                        )
                        .child(
                            icon_button(
                                "cal-next",
                                IconName::ChevronRight,
                                p.muted,
                                p.hover,
                                14.0,
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.model.week_start = this.model.week_start + Duration::days(7);
                                cx.notify();
                            })),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(10.0))
                .child(
                    div()
                        .flex()
                        .h(px(30.0))
                        .px(px(3.0))
                        .items_center()
                        .rounded_full()
                        .bg(p.inset.with_alpha(if p.is_dark { 0.9 } else { 1.0 }).gpui())
                        .border_1()
                        .border_color(p.border.with_alpha(0.55).gpui())
                        .children(["Day", "Week", "Month"].iter().enumerate().map(
                            |(index, label)| {
                                let active = index == 1;
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .h(px(24.0))
                                    .px(px(12.0))
                                    .rounded_full()
                                    .when(active, |this| {
                                        this.bg(p.surface.gpui()).shadow_sm()
                                    })
                                    .text_size(px(11.0))
                                    .font_weight(if active {
                                        FontWeight::MEDIUM
                                    } else {
                                        FontWeight::NORMAL
                                    })
                                    .text_color(if active { p.text } else { p.muted }.gpui())
                                    .child(*label)
                            },
                        )),
                )
                .child(
                    icon_button(
                        "cal-sync",
                        IconName::RefreshCw,
                        p.muted,
                        p.hover,
                        14.0,
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.sync_calendar(cx))),
                )
                .child(
                    div()
                        .id("cal-today")
                        .flex()
                        .items_center()
                        .h(px(30.0))
                        .px(px(11.0))
                        .rounded_full()
                        .border_1()
                        .border_color(p.border.with_alpha(0.6).gpui())
                        .cursor_pointer()
                        .text_size(px(11.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(p.muted.gpui())
                        .hover(|style| style.bg(p.hover.gpui()))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.model.week_start = week_start(this.model.today);
                            cx.notify();
                        }))
                        .child("Today"),
                )
                .child(
                    div()
                        .id("cal-new")
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .h(px(30.0))
                        .px(px(12.0))
                        .rounded_full()
                        .bg(p.accent.gpui())
                        .text_size(px(11.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(p.accent_ink.gpui())
                        .cursor_pointer()
                        .hover(|style| style.opacity(0.9))
                        .on_click(cx.listener(|this, _, _, cx| {
                            let today = this.model.today;
                            this.model.open_event_sheet(None, today, 9 * 60, 10 * 60);
                            cx.notify();
                        }))
                        .child(icon(IconName::Plus, 12.0, p.accent_ink))
                        .child("New event"),
                ),
        )
}

fn week_header(app: &CleanApp, days: &[chrono::NaiveDate]) -> impl IntoElement {
    let model = &app.model;
    let p = &model.palette;

    div()
        .flex()
        .px(px(18.0))
        .pb(px(8.0))
        .child(div().w(px(GUTTER)).flex_shrink_0())
        .child(
            div()
                .flex()
                .flex_1()
                .min_w(px(0.0))
                .children(days.iter().map(|day| {
                    let is_today = *day == model.today;
                    let count = model.events.iter().filter(|event| event.day == *day).count();
                    div()
                        .flex()
                        .flex_1()
                        .flex_col()
                        .items_center()
                        .gap(px(3.0))
                        .py(px(7.0))
                        .rounded(px(10.0))
                        .when(is_today, |this| this.bg(p.accent.with_alpha(0.06).gpui()))
                        .child(
                            div()
                                .text_size(px(9.5))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(if is_today { p.accent } else { p.faint }.gpui())
                                .child(day.format("%a").to_string().to_uppercase()),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .size(px(26.0))
                                .rounded_full()
                                .when(is_today, |this| this.bg(p.accent.gpui()))
                                .text_size(px(13.0))
                                .font_weight(if is_today {
                                    FontWeight::SEMIBOLD
                                } else {
                                    FontWeight::MEDIUM
                                })
                                .text_color(if is_today { p.accent_ink } else { p.text }.gpui())
                                .child(day.day().to_string()),
                        )
                        .child(
                            div()
                                .text_size(px(9.0))
                                .text_color(p.faint.gpui())
                                .child(if count == 0 {
                                    "—".to_string()
                                } else {
                                    format!("{count} event{}", if count == 1 { "" } else { "s" })
                                }),
                        )
                })),
        )
}

fn time_gutter(p: &Palette) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .w(px(GUTTER))
        .flex_shrink_0()
        .pt(px(6.0))
        .children((START_HOUR..END_HOUR).map(|hour| {
            div()
                .flex()
                .justify_end()
                .pr(px(10.0))
                .h(px(HOUR_HEIGHT))
                .text_size(px(9.5))
                .text_color(p.faint.gpui())
                .child(format_clock(hour * 60))
        }))
}

fn day_column(
    _app: &CleanApp,
    model: &Model,
    p: &Palette,
    day: chrono::NaiveDate,
    last: bool,
    cx: &mut Context<CleanApp>,
) -> AnyElement {
    let events: Vec<&CalendarEvent> = model
        .events
        .iter()
        .filter(|event| event.day == day)
        .collect();
    let is_today = day == model.today;
    let now_offset = now_minutes() as f32;
    let show_now = is_today
        && now_offset >= START_HOUR as f32 * 60.0
        && now_offset <= END_HOUR as f32 * 60.0;
    let now_top = ((now_offset - START_HOUR as f32 * 60.0) / 60.0) * HOUR_HEIGHT;

    div()
        .relative()
        .flex()
        .flex_col()
        .flex_1()
        .min_w(px(0.0))
        .border_l(px(1.0))
        .border_color(p.border.with_alpha(0.45).gpui())
        .when(last, |this| this.border_r(px(0.0)))
        .child(
            div()
                .relative()
                .flex()
                .flex_col()
                .pt(px(6.0))
                .children((START_HOUR..END_HOUR).map(|_| {
                    div()
                        .h(px(HOUR_HEIGHT))
                        .w_full()
                        .border_t(px(1.0))
                        .border_color(p.border.with_alpha(0.35).gpui())
                })),
        )
        .when(show_now, |this| {
            this.child(
                div()
                    .absolute()
                    .left(px(0.0))
                    .right(px(0.0))
                    .top(px(now_top + 6.0))
                    .flex()
                    .items_center()
                    .child(div().flex_1().h(px(1.5)).bg(p.danger.gpui()))
                    .child(
                        div()
                            .size(px(6.0))
                            .rounded_full()
                            .bg(p.danger.gpui())
                            .ml(px(-3.0)),
                    ),
            )
        })
        .children(events.iter().enumerate().map(|(stack, event)| {
            let start = event.start.max(START_HOUR * 60);
            let end = event.end.min(END_HOUR * 60);
            let top = 6.0 + ((start - START_HOUR * 60) as f32 / 60.0) * HOUR_HEIGHT
                + stack as f32 * 3.0;
            let height = (((end.saturating_sub(start)) as f32 / 60.0) * HOUR_HEIGHT - 4.0)
                .max(20.0);
            let color = event_color(event.color, p.is_dark);
            let event_id = event.id;

            div()
                .id(("cal-event", event.id))
                .absolute()
                .left(px(4.0))
                .right(px(4.0))
                .top(px(top))
                .h(px(height))
                .flex()
                .flex_col()
                .gap(px(1.0))
                .px(px(6.0))
                .py(px(3.0))
                .rounded(px(6.0))
                .bg(color.with_alpha(if p.is_dark { 0.18 } else { 0.13 }).gpui())
                .border_l(px(2.5))
                .border_color(color.gpui())
                .overflow_hidden()
                .cursor_pointer()
                .hover(|style| style.bg(color.with_alpha(0.28).gpui()))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.delete_calendar_event(event_id, cx);
                }))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.0))
                        .child(
                            div()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .text_size(px(10.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(color.ensure_contrast(p.surface, 2.4).gpui())
                                .child(format!("{} {}", event.emoji, event.title)),
                        ),
                )
                .child(
                    div()
                        .text_size(px(9.0))
                        .text_color(p.muted.gpui())
                        .child(event.time_label()),
                )
        }))
        .into_any_element()
}

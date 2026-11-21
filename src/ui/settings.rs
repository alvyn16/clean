use gpui_kit::*;

use crate::theme::{Palette, Rgba, ThemeMode};
use crate::ui::model::SetupStep;
use crate::ui::widgets::*;
use crate::ui::{CleanApp, Field, Screen};

const TABS: [(&str, IconName); 6] = [
    ("Appearance", IconName::Palette),
    ("Accounts", IconName::User),
    ("Calendar", IconName::Calendar),
    ("Notifications", IconName::Bell),
    ("Shortcuts", IconName::Keyboard),
    ("About", IconName::Info),
];

pub fn sheet(app: &CleanApp, cx: &mut Context<CleanApp>) -> impl IntoElement {
    let p = &app.model.palette;

    div()
        .absolute()
        .top(px(0.0))
        .left(px(0.0))
        .right(px(0.0))
        .bottom(px(0.0))
        .flex()
        .items_center()
        .justify_center()
        .bg(Rgba::hex(0x000000).with_alpha(if p.is_dark { 0.45 } else { 0.22 }).gpui())
        .child(
            div()
                .flex()
                .flex_col()
                .w(px(760.0))
                .h(px(540.0))
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
                        .flex_1()
                        .min_h(px(0.0))
                        .child(nav(app, cx))
                        .child(content(app, cx)),
                ),
        )
}

fn header(app: &CleanApp, cx: &mut Context<CleanApp>) -> impl IntoElement {
    let p = &app.model.palette;

    div()
        .flex()
        .items_center()
        .justify_between()
        .h(px(52.0))
        .px(px(18.0))
        .border_b(px(1.0))
        .border_color(p.border.with_alpha(0.6).gpui())
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .font_family("Instrument Serif")
                        .text_size(px(20.0))
                        .text_color(p.text.gpui())
                        .child("Settings"),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(p.faint.gpui())
                        .child("clean"),
                ),
        )
        .child(
            icon_button("settings-close", IconName::X, p.muted, p.hover, 14.0).on_click(
                cx.listener(|this, _, _, cx| {
                    this.model.show_settings = false;
                    cx.notify();
                }),
            ),
        )
}

fn nav(app: &CleanApp, cx: &mut Context<CleanApp>) -> impl IntoElement {
    let p = &app.model.palette;
    let selected = app.settings_tab;

    div()
        .flex()
        .flex_col()
        .gap(px(2.0))
        .w(px(190.0))
        .flex_shrink_0()
        .p(px(10.0))
        .bg(p.sidebar.gpui())
        .border_r(px(1.0))
        .border_color(p.border.with_alpha(0.5).gpui())
        .children(TABS.iter().enumerate().map(|(index, (label, icon_name))| {
            let active = index == selected;
            div()
                .id(("settings-tab", index))
                .flex()
                .items_center()
                .gap(px(9.0))
                .h(px(32.0))
                .px(px(9.0))
                .rounded(px(7.0))
                .cursor_pointer()
                .when(active, |this| this.bg(p.selection.gpui()))
                .when(!active, |this| {
                    this.hover(|style| style.bg(p.hover.gpui()))
                })
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.settings_tab = index;
                    cx.notify();
                }))
                .child(icon(
                    *icon_name,
                    14.0,
                    if active { p.accent } else { p.muted },
                ))
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_weight(if active {
                            FontWeight::MEDIUM
                        } else {
                            FontWeight::NORMAL
                        })
                        .text_color(if active { p.text } else { p.muted }.gpui())
                        .child(*label),
                )
        }))
        .child(div().flex_1())
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .px(px(9.0))
                .py(px(7.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(px(22.0))
                        .rounded_full()
                        .bg(p.accent.with_alpha(0.16).gpui())
                        .child(icon(IconName::Leaf, 11.0, p.accent)),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(1.0))
                        .child(
                            div()
                                .text_size(px(10.5))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(p.muted.gpui())
                                .child("v0.1.0"),
                        )
                        .child(
                            div()
                                .text_size(px(9.5))
                                .text_color(p.faint.gpui())
                                .child(omarchy_label(app)),
                        ),
                ),
        )
}

fn omarchy_label(app: &CleanApp) -> String {
    match crate::theme::omarchy::current_theme_name() {
        Some(name) => name,
        None => {
            let _ = app;
            "Omarchy".into()
        }
    }
}

fn content(app: &CleanApp, cx: &mut Context<CleanApp>) -> impl IntoElement {
    let p = &app.model.palette;

    div()
        .id("settings-content")
        .flex()
        .flex_col()
        .flex_1()
        .min_w(px(0.0))
        .h_full()
        .overflow_y_scroll()
        .px(px(24.0))
        .py(px(20.0))
        .gap(px(18.0))
        .child(match app.settings_tab {
            0 => appearance(app, cx).into_any_element(),
            1 => accounts(app, cx).into_any_element(),
            2 => calendar(app, cx).into_any_element(),
            3 => notifications(app, cx).into_any_element(),
            4 => shortcuts(app).into_any_element(),
            _ => about(app, cx).into_any_element(),
        })
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.0))
                .pt(px(6.0))
                .child(icon(IconName::Shield, 11.0, p.faint))
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(p.faint.gpui())
                        .child("Credentials live in your system keyring, never in config files."),
                ),
        )
}

fn section(p: &Palette, title: &'static str, body: impl IntoElement) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(10.0))
        .child(
            div()
                .text_size(px(11.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(p.text.gpui())
                .child(title),
        )
        .child(body)
}

fn appearance(app: &CleanApp, cx: &mut Context<CleanApp>) -> impl IntoElement {
    let model = &app.model;
    let p = &model.palette;
    let mode = model.config.appearance.mode;

    div()
        .flex()
        .flex_col()
        .gap(px(20.0))
        .child(section(
            p,
            "Theme",
            div()
                .flex()
                .gap(px(9.0))
                .children(ThemeMode::ALL.iter().map(|candidate| {
                    theme_option(app, *candidate, mode == *candidate).on_click(cx.listener(
                        move |this, _, _, cx| this.set_mode(*candidate, cx),
                    ))
                })),
        ))
        .child(section(
            p,
            "Accent",
            div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(9.0))
                        .children(crate::theme::Accent::SWATCHES.iter().map(|candidate| {
                            accent_option(app, *candidate, model.accent() == *candidate).on_click(
                                cx.listener(move |this, _, _, cx| this.set_accent(*candidate, cx)),
                            )
                        })),
                )
                .child(text(
                    if matches!(model.accent(), crate::theme::Accent::Blue)
                        && mode == ThemeMode::Omarchy
                    {
                        "Following Omarchy's live blue token"
                    } else {
                        "Accent applies to selection, buttons and highlights"
                    },
                    10.5,
                    FontWeight::NORMAL,
                    p.faint,
                )),
        ))
        .child(section(
            p,
            "Reading",
            div()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(setting_row(
                    p,
                    "Reduce motion",
                    "Use instant transitions instead of springs",
                    switch(p, !model.config.appearance.reduce_motion),
                ))
                .child(setting_row(
                    p,
                    "Text size",
                    "Comfortable · 13pt",
                    chip("Default", p.muted, p.inset),
                )),
        ))
}

fn provider_icon(p: &Palette, account: &crate::config::AccountConfig) -> impl IntoElement {
    let icon_name = match account.provider {
        crate::config::Provider::Google => IconName::Globe,
        crate::config::Provider::Apple => IconName::Apple,
        crate::config::Provider::Proton => IconName::Shield,
    };
    div()
        .flex()
        .items_center()
        .justify_center()
        .size(px(28.0))
        .rounded(px(7.0))
        .bg(p.accent.with_alpha(0.14).gpui())
        .child(icon(icon_name, 13.0, p.accent))
}

fn theme_option(app: &CleanApp, mode: ThemeMode, selected: bool) -> Stateful<Div> {
    let model = &app.model;
    let p = &model.palette;
    let accent = model
        .accent()
        .resolve(mode, model.omarchy.as_ref().map(|colors| colors.blue()));
    let preview = match mode {
        ThemeMode::White => Palette::white(accent),
        ThemeMode::Black => Palette::black(accent),
        ThemeMode::Omarchy => model
            .omarchy
            .as_ref()
            .map(|colors| colors.palette(accent))
            .unwrap_or_else(|| Palette::black(accent)),
    };

    div()
        .id(ElementId::Name(
            format!("settings-theme-{}", mode.label()).into(),
        ))
        .flex()
        .flex_col()
        .gap(px(7.0))
        .w(px(118.0))
        .p(px(8.0))
        .rounded(px(10.0))
        .bg(p.surface.gpui())
        .border_1()
        .border_color(if selected {
            p.accent.gpui()
        } else {
            p.border.with_alpha(0.65).gpui()
        })
        .cursor_pointer()
        .hover(|style| style.border_color(p.accent.with_alpha(0.4).gpui()))
        .child(
            div()
                .flex()
                .flex_col()
                .h(px(46.0))
                .rounded(px(6.0))
                .overflow_hidden()
                .bg(preview.canvas.gpui())
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(2.5))
                        .h(px(10.0))
                        .px(px(5.0))
                        .bg(preview.sidebar.gpui())
                        .children((0..3).map(|_| {
                            div()
                                .size(px(2.0))
                                .rounded_full()
                                .bg(preview.faint.gpui())
                        })),
                )
                .child(
                    div()
                        .flex()
                        .flex_1()
                        .child(div().w(px(22.0)).bg(preview.sidebar.gpui()))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(3.0))
                                .flex_1()
                                .p(px(5.0))
                                .children((0..3).map(|index| {
                                    div()
                                        .h(px(3.0))
                                        .w(px(if index == 0 { 26.0 } else { 38.0 }))
                                        .rounded_full()
                                        .bg(if index == 0 {
                                            preview.accent.gpui()
                                        } else {
                                            preview.faint.with_alpha(0.3).gpui()
                                        })
                                })),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .px(px(2.0))
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_weight(if selected {
                            FontWeight::MEDIUM
                        } else {
                            FontWeight::NORMAL
                        })
                        .text_color(if selected { p.text } else { p.muted }.gpui())
                        .child(mode.label()),
                )
                .when(selected, |this| {
                    this.child(icon(IconName::Check, 11.0, p.accent))
                }),
        )
}

fn accent_option(app: &CleanApp, accent: crate::theme::Accent, selected: bool) -> Stateful<Div> {
    let p = &app.model.palette;
    let color = accent.resolve(
        app.model.config.appearance.mode,
        app.model.omarchy.as_ref().map(|colors| colors.blue()),
    );

    div()
        .id(ElementId::Name(
            format!("settings-accent-{}", accent.label()).into(),
        ))
        .flex()
        .items_center()
        .justify_center()
        .size(px(24.0))
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
                .flex()
                .items_center()
                .justify_center()
                .size(px(16.0))
                .rounded_full()
                .bg(color.gpui())
                .when(selected, |this| {
                    this.child(icon(IconName::Check, 9.0, color.best_ink()))
                }),
        )
}

fn accounts(app: &CleanApp, cx: &mut Context<CleanApp>) -> impl IntoElement {
    let model = &app.model;
    let p = &model.palette;

    div()
        .flex()
        .flex_col()
        .gap(px(16.0))
        .child(section(
            p,
            "Connected accounts",
            div().flex().flex_col().gap(px(7.0)).children(
                model
                    .config
                    .accounts
                    .iter()
                    .map(|account| {
                        let id = account.id.clone();
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.0))
                            .p(px(11.0))
                            .rounded(px(9.0))
                            .bg(p.surface.gpui())
                            .border_1()
                            .border_color(p.border.with_alpha(0.6).gpui())
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .size(px(28.0))
                                    .rounded(px(7.0))
                                    .bg(p.accent.with_alpha(0.14).gpui())
                                    .child(
                                        if let Some(path) = app.avatar_for(&account.id) {
                                            img(path)
                                                .id(ElementId::Name(
                                                    format!("settings-avatar-{}", account.id).into(),
                                                ))
                                                .size(px(28.0))
                                                .rounded_full()
                                                .object_fit(gpui_kit::ObjectFit::Cover)
                                                .into_any_element()
                                        } else {
                                            provider_icon(p, account).into_any_element()
                                        },
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(1.0))
                                    .flex_1()
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(p.text.gpui())
                                            .child(account.email.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.5))
                                            .text_color(p.faint.gpui())
                                            .child(format!(
                                                "{} · {}",
                                                account.provider.label(),
                                                account.imap_host
                                            )),
                                    ),
                            )
                            .child(
                                div()
                                    .id(ElementId::Name(
                                        format!("disconnect-{}", account.id).into(),
                                    ))
                                    .flex()
                                    .items_center()
                                    .h(px(26.0))
                                    .px(px(9.0))
                                    .rounded(px(6.0))
                                    .cursor_pointer()
                                    .text_size(px(10.5))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(p.muted.gpui())
                                    .hover(|style| {
                                        style
                                            .bg(p.danger.with_alpha(0.10).gpui())
                                            .text_color(p.danger.gpui())
                                    })
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.disconnect_account(id.clone(), cx);
                                    }))
                                    .child("Disconnect"),
                            )
                    })
                    .collect::<Vec<_>>()
                    .into_iter(),
            ),
        ))
        .when(model.config.accounts.is_empty(), |this| {
            this.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .p(px(13.0))
                    .rounded(px(9.0))
                    .bg(p.inset.with_alpha(0.6).gpui())
                    .child(text(
                        "You're browsing demo mail",
                        12.0,
                        FontWeight::MEDIUM,
                        p.text,
                    ))
                    .child(text(
                        "Connect Google, Apple or Proton to sync a real inbox.",
                        11.0,
                        FontWeight::NORMAL,
                        p.faint,
                    )),
            )
        })
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .id("settings-add-account")
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .h(px(32.0))
                        .px(px(12.0))
                        .rounded(px(8.0))
                        .bg(p.accent.gpui())
                        .text_size(px(11.5))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(p.accent_ink.gpui())
                        .cursor_pointer()
                        .hover(|style| style.opacity(0.92))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.model.show_settings = false;
                            this.model.screen = Screen::Setup;
                            this.model.setup_step = SetupStep::Account;
                            this.model.sign_in.provider = Some(crate::config::Provider::Google);
                            cx.notify();
                        }))
                        .child(icon(IconName::Plus, 12.0, p.accent_ink))
                        .child("Add account"),
                )
                .child(text(
                    "Google needs a Desktop OAuth client · Proton needs Bridge",
                    10.5,
                    FontWeight::NORMAL,
                    p.faint,
                )),
        )
        .child(section(
            p,
            "Unified inbox",
            div()
                .flex()
                .items_center()
                .gap(px(12.0))
                .h(px(52.0))
                .px(px(10.0))
                .rounded(px(8.0))
                .bg(p.surface.gpui())
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .gap(px(2.0))
                        .child(text(
                            "All Inboxes",
                            12.0,
                            FontWeight::MEDIUM,
                            p.text,
                        ))
                        .child(text(
                            "Merge the inboxes of every connected account",
                            10.5,
                            FontWeight::NORMAL,
                            p.faint,
                        )),
                )
                .child(
                    div()
                        .id("unified-inbox-toggle")
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.toggle_unified_inbox(cx);
                        }))
                        .child(switch(p, model.config.unified_inbox)),
                ),
        ))
}

fn notifications(app: &CleanApp, cx: &mut Context<CleanApp>) -> impl IntoElement {
    let p = &app.model.palette;
    let flags = app.notifications;

    div()
        .flex()
        .flex_col()
        .gap(px(16.0))
        .child(section(
            p,
            "Alerts",
            div()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(toggle_row(
                    p,
                    0,
                    "New mail",
                    "Show a desktop notification for the inbox",
                    flags[0],
                    cx,
                ))
                .child(toggle_row(
                    p,
                    1,
                    "Sounds",
                    "A quiet chime when mail arrives",
                    flags[1],
                    cx,
                ))
                .child(toggle_row(
                    p,
                    2,
                    "Calendar reminders",
                    "Ten minutes before an event",
                    flags[2],
                    cx,
                )),
        ))
        .child(
            text(
                "Notifications respect your desktop's Do Not Disturb state.",
                10.5,
                FontWeight::NORMAL,
                p.faint,
            ),
        )
}

fn toggle_row(
    p: &Palette,
    index: usize,
    title: &'static str,
    subtitle: &'static str,
    on: bool,
    cx: &mut Context<CleanApp>,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(12.0))
        .h(px(52.0))
        .px(px(10.0))
        .rounded(px(8.0))
        .hover(|style| style.bg(p.hover.gpui()))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .flex_1()
                .child(text(title, 12.0, FontWeight::MEDIUM, p.text))
                .child(text(subtitle, 10.5, FontWeight::NORMAL, p.faint)),
        )
        .child(
            div()
                .id(("toggle", index))
                .flex()
                .items_center()
                .w(px(36.0))
                .h(px(21.0))
                .px(px(2.5))
                .rounded_full()
                .cursor_pointer()
                .when(on, |this| this.bg(p.accent.gpui()).justify_end())
                .when(!on, |this| {
                    this.bg(p.border.gpui()).justify_start()
                })
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.notifications[index] = !this.notifications[index];
                    cx.notify();
                }))
                .child(
                    div()
                        .size(px(16.0))
                        .rounded_full()
                        .bg(Rgba::hex(0xFFFFFF).gpui())
                        .shadow_sm(),
                ),
        )
}

fn calendar_field(
    app: &CleanApp,
    id: &'static str,
    field: Field,
    label: &'static str,
    value: &str,
    placeholder: &str,
    password: bool,
    cx: &mut Context<CleanApp>,
) -> impl IntoElement {
    let p = &app.model.palette;
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(10.0))
        .h(px(38.0))
        .cursor_text()
        .on_click(cx.listener(move |this, _, window, cx| {
            this.focus_field(field, window, cx);
        }))
        .child(
            div()
                .w(px(74.0))
                .flex_shrink_0()
                .text_size(px(11.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(p.faint.gpui())
                .child(label),
        )
        .child(field_value(
            value,
            placeholder,
            app.model.focused == Some(field),
            password,
            p,
        ))
}

fn calendar(app: &CleanApp, cx: &mut Context<CleanApp>) -> impl IntoElement {
    let model = &app.model;
    let p = &model.palette;

    div()
        .flex()
        .flex_col()
        .gap(px(16.0))
        .child(section(
            p,
            "CalDAV calendar",
            div()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .child(calendar_field(
                    app,
                    "calendar-url",
                    Field::CalendarUrl,
                    "URL",
                    &model.calendar_url,
                    "https://caldav.example.com/calendars/me/",
                    false,
                    cx,
                ))
                .child(div().h(px(1.0)).w_full().bg(p.border.with_alpha(0.4).gpui()))
                .child(calendar_field(
                    app,
                    "calendar-user",
                    Field::CalendarUser,
                    "Username",
                    &model.calendar_user,
                    "me@example.com",
                    false,
                    cx,
                ))
                .child(div().h(px(1.0)).w_full().bg(p.border.with_alpha(0.4).gpui()))
                .child(calendar_field(
                    app,
                    "calendar-password",
                    Field::CalendarPassword,
                    "Password",
                    &model.calendar_password,
                    "App password (stored in keyring)",
                    true,
                    cx,
                )),
        ))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(10.0))
                .child(
                    div()
                        .id("calendar-save")
                        .flex()
                        .items_center()
                        .h(px(30.0))
                        .px(px(13.0))
                        .rounded(px(7.0))
                        .bg(p.accent.gpui())
                        .cursor_pointer()
                        .text_size(px(11.5))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(p.accent_ink.gpui())
                        .hover(|style| style.opacity(0.92))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.save_calendar_settings(cx);
                        }))
                        .child("Save & sync"),
                )
                .child(
                    div()
                        .id("calendar-sync")
                        .flex()
                        .items_center()
                        .h(px(30.0))
                        .px(px(13.0))
                        .rounded(px(7.0))
                        .bg(p.inset.gpui())
                        .cursor_pointer()
                        .text_size(px(11.5))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(p.text.gpui())
                        .hover(|style| style.bg(p.hover.gpui()))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.sync_calendar(cx);
                        }))
                        .child("Sync now"),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .child(text(
                    model.calendar_status.clone(),
                    11.0,
                    FontWeight::NORMAL,
                    p.muted,
                ))
                .child(text(
                    "Events are stored locally and mirrored to your CalDAV calendar. \
                     Password is kept in the system keyring.",
                    10.5,
                    FontWeight::NORMAL,
                    p.faint,
                )),
        )
}

fn shortcuts(app: &CleanApp) -> impl IntoElement {
    let p = &app.model.palette;
    let rows = [
        ("Compose", "Ctrl N"),
        ("Search", "Ctrl K"),
        ("Settings", "Ctrl ,"),
        ("Next / previous message", "J  /  K"),
        ("Open message", "Enter"),
        ("Archive message", "E"),
        ("Delete message", "#  or  Del"),
        ("Mark read / unread", "U"),
        ("Sync now", "Ctrl R"),
        ("Copy selected text", "Ctrl C"),
        ("Select all text", "Ctrl A"),
        ("Mail / Calendar", "V  or  Ctrl M"),
        ("Next field", "Tab"),
        ("Close overlay", "Esc"),
    ];

    div().flex().flex_col().gap(px(16.0)).child(section(
        p,
        "Keyboard",
        div()
            .flex()
            .flex_col()
            .children(rows.iter().map(|(label, keys)| {
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .h(px(38.0))
                    .border_b(px(1.0))
                    .border_color(p.border.with_alpha(0.35).gpui())
                    .child(text(*label, 11.5, FontWeight::NORMAL, p.muted))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .h(px(21.0))
                            .px(px(7.0))
                            .rounded(px(5.0))
                            .bg(p.inset.with_alpha(0.8).gpui())
                            .border_1()
                            .border_color(p.border.with_alpha(0.6).gpui())
                            .text_size(px(10.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(p.text.gpui())
                            .child(*keys),
                    )
            })),
    ))
}

fn about(app: &CleanApp, cx: &mut Context<CleanApp>) -> impl IntoElement {
    let p = &app.model.palette;

    div()
        .flex()
        .flex_col()
        .gap(px(18.0))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(
                    div()
                        .flex()
                        .items_baseline()
                        .gap(px(6.0))
                        .child(serif(34.0, FontWeight::NORMAL, p.text).child("clean")),
                )
                .child(text(
                    "Mail, without the noise. A calm desktop client for Linux.",
                    12.0,
                    FontWeight::NORMAL,
                    p.muted,
                ))
                .child(text(
                    "Version 0.1.0 · built with Rust and GPUI Kit",
                    10.5,
                    FontWeight::NORMAL,
                    p.faint,
                )),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .children(
                    [
                        ("gpui-kit.com", IconName::ArrowUpRight),
                        ("Report an issue", IconName::Github),
                        ("Made for Omarchy", IconName::Heart),
                    ]
                    .iter()
                    .map(|(label, icon_name)| {
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .h(px(30.0))
                            .px(px(11.0))
                            .rounded(px(8.0))
                            .border_1()
                            .border_color(p.border.with_alpha(0.6).gpui())
                            .text_size(px(11.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(p.muted.gpui())
                            .cursor_pointer()
                            .hover(|style| style.bg(p.hover.gpui()))
                            .child(*label)
                            .child(icon(*icon_name, 11.0, p.faint))
                    }),
                ),
        )
        .child(section(
            p,
            "Testing",
            div().flex().flex_col().gap(px(8.0)).child(
                div()
                    .id("settings-replay-setup")
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .h(px(34.0))
                    .px(px(12.0))
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(p.border.with_alpha(0.6).gpui())
                    .cursor_pointer()
                    .text_size(px(11.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(p.muted.gpui())
                    .hover(|style| style.bg(p.hover.gpui()))
                    .on_click(cx.listener(|this, _, _, cx| this.replay_setup(cx)))
                    .child(icon(IconName::RotateCcw, 12.0, p.accent))
                    .child("Replay first-run setup"),
            ),
        ))
}

fn setting_row(
    p: &Palette,
    title: &'static str,
    subtitle: &'static str,
    trailing: impl IntoElement,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(12.0))
        .min_h(px(52.0))
        .px(px(10.0))
        .rounded(px(8.0))
        .hover(|style| style.bg(p.hover.gpui()))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .flex_1()
                .child(text(title, 12.0, FontWeight::MEDIUM, p.text))
                .child(text(subtitle, 10.5, FontWeight::NORMAL, p.faint)),
        )
        .child(trailing)
}

fn switch(p: &Palette, on: bool) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .w(px(34.0))
        .h(px(20.0))
        .px(px(2.0))
        .rounded_full()
        .bg(if on { p.accent.gpui() } else { p.border.gpui() })
        .when(on, |this| this.justify_end())
        .when(!on, |this| this.justify_start())
        .child(
            div()
                .size(px(16.0))
                .rounded_full()
                .bg(Rgba::hex(0xFFFFFF).gpui())
                .shadow_sm(),
        )
}

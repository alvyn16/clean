use gpui_kit::*;

use crate::config::Provider;
use crate::theme::{Accent, Palette, Rgba, ThemeMode};
use crate::ui::model::{Field, SetupStep};
use crate::ui::widgets::*;
use crate::ui::CleanApp;

pub fn setup(app: &CleanApp, cx: &mut Context<CleanApp>) -> impl IntoElement {
    let model = &app.model;
    let p = &model.palette;

    div()
        .flex()
        .size_full()
        .bg(p.canvas.gpui())
        .text_color(p.text.gpui())
        .child(brand_panel(app))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w(px(520.0))
                .h_full()
                .items_center()
                .justify_center()
                .px(px(48.0))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .w(px(480.0))
                        .gap(px(24.0))
                        .child(step_heading(app))
                        .child(match model.setup_step {
                            SetupStep::Appearance => appearance_step(app, cx).into_any_element(),
                            SetupStep::Account => account_step(app, cx).into_any_element(),
                            SetupStep::Ready => ready_step(app, cx).into_any_element(),
                        })
                        .child(footer(app, cx)),
                ),
        )
}

fn brand_panel(app: &CleanApp) -> impl IntoElement {
    let p = &app.model.palette;
    let deep = p.accent.mix(if p.is_dark {
        Rgba::hex(0x0B0C0E)
    } else {
        Rgba::hex(0x111827)
    }, 0.72);

    div()
        .relative()
        .flex()
        .flex_col()
        .justify_between()
        .w(px(520.0))
        .h_full()
        .flex_shrink_0()
        .px(px(48.0))
        .py(px(48.0))
        .bg(deep.gpui())
        .text_color(Rgba::hex(0xFFFFFF).gpui())
        .on_mouse_down(MouseButton::Left, |_, window, _| {
            window.start_window_move();
        })
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(10.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(px(42.0))
                        .rounded(px(10.0))
                        .bg(Rgba::hex(0xFFFFFF).with_alpha(0.14).gpui())
                        .child(icon(
                            IconName::Leaf,
                            22.0,
                            Rgba::hex(0xFFFFFF).with_alpha(0.9),
                        )),
                )
                .child(
                    div()
                        .font_family("Instrument Serif")
                        .text_size(px(34.0))
                        .text_color(Rgba::hex(0xFFFFFF).gpui())
                        .child("clean"),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(18.0))
                .child(
                    div()
                        .font_family("Instrument Serif")
                        .text_size(px(64.0))
                        .line_height(px(70.0))
                        .text_color(Rgba::hex(0xFFFFFF).gpui())
                        .child("Mail, without\nthe noise."),
                )
                .child(
                    div()
                        .max_w(px(400.0))
                        .text_size(px(15.5))
                        .line_height(px(24.0))
                        .text_color(Rgba::hex(0xFFFFFF).with_alpha(0.66).gpui())
                        .child(
                            "A calm, fast mail client for people who like their inbox quiet. \
                             Native on Linux, themed by Omarchy, privacy first.",
                        ),
                )
                .child(preview_card(p)),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(14.0))
                .text_size(px(12.5))
                .text_color(Rgba::hex(0xFFFFFF).with_alpha(0.45).gpui())
                .child("IMAP · SMTP")
                .child(dot(Rgba::hex(0xFFFFFF).with_alpha(0.25), 3.0))
                .child("Keyring")
                .child(dot(Rgba::hex(0xFFFFFF).with_alpha(0.25), 3.0))
                .child("Local cache"),
        )
}

fn preview_card(p: &Palette) -> impl IntoElement {
    let card = Rgba::hex(0xFFFFFF).with_alpha(if p.is_dark { 0.07 } else { 0.10 });
    let line = Rgba::hex(0xFFFFFF).with_alpha(0.16);

    div()
        .mt(px(16.0))
        .flex()
        .flex_col()
        .gap(px(12.0))
        .p(px(20.0))
        .rounded(px(16.0))
        .bg(card.gpui())
        .border_1()
        .border_color(line.gpui())
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(px(30.0))
                        .rounded_full()
                        .bg(p.accent.gpui())
                        .text_size(px(13.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(p.accent_ink.gpui())
                        .child("M"),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(4.0))
                        .child(
                            div()
                                .w(px(150.0))
                                .h(px(8.0))
                                .rounded_full()
                                .bg(Rgba::hex(0xFFFFFF).with_alpha(0.55).gpui()),
                        )
                        .child(
                            div()
                                .w(px(95.0))
                                .h(px(6.0))
                                .rounded_full()
                                .bg(Rgba::hex(0xFFFFFF).with_alpha(0.22).gpui()),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(
                    div()
                        .w_full()
                        .h(px(6.0))
                        .rounded_full()
                        .bg(Rgba::hex(0xFFFFFF).with_alpha(0.18).gpui()),
                )
                .child(
                    div()
                        .w(px(270.0))
                        .h(px(6.0))
                        .rounded_full()
                        .bg(Rgba::hex(0xFFFFFF).with_alpha(0.18).gpui()),
                )
                .child(
                    div()
                        .w(px(210.0))
                        .h(px(6.0))
                        .rounded_full()
                        .bg(Rgba::hex(0xFFFFFF).with_alpha(0.18).gpui()),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .h(px(24.0))
                        .px(px(12.0))
                        .rounded_full()
                        .bg(p.accent.with_alpha(0.85).gpui())
                        .text_size(px(11.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(p.accent_ink.gpui())
                        .flex()
                        .items_center()
                        .child("Reply"),
                )
                .child(
                    div()
                        .h(px(24.0))
                        .px(px(12.0))
                        .rounded_full()
                        .bg(Rgba::hex(0xFFFFFF).with_alpha(0.12).gpui())
                        .text_size(px(11.0))
                        .text_color(Rgba::hex(0xFFFFFF).with_alpha(0.7).gpui())
                        .flex()
                        .items_center()
                        .child("Archive"),
                ),
        )
}

fn step_heading(app: &CleanApp) -> impl IntoElement {
    let model = &app.model;
    let p = &model.palette;
    let (title, subtitle) = match model.setup_step {
        SetupStep::Appearance => (
            "Welcome to clean",
            "Pick a look. You can change it any time in Settings.",
        ),
        SetupStep::Account => (
            "Connect your mail",
            "Your credentials stay in your system keyring. Nothing leaves this machine.",
        ),
        SetupStep::Ready => ("You're all set", "Your inbox is ready when you are."),
    };

    div()
        .flex()
        .flex_col()
        .gap(px(6.0))
        .child(
            div()
                .font_family("Instrument Serif")
                .text_size(px(42.0))
                .text_color(p.text.gpui())
                .child(title),
        )
        .child(text(subtitle, 14.0, FontWeight::NORMAL, p.muted))
        .child(progress(app))
}

fn progress(app: &CleanApp) -> impl IntoElement {
    let model = &app.model;
    let p = &model.palette;
    let step = match model.setup_step {
        SetupStep::Appearance => 0,
        SetupStep::Account => 1,
        SetupStep::Ready => 2,
    };

    div()
        .flex()
        .items_center()
        .gap(px(5.0))
        .pt(px(6.0))
        .children((0..3).map(|index| {
            let active = index <= step;
            div()
                .h(px(3.0))
                .w(px(if index == step { 26.0 } else { 14.0 }))
                .rounded_full()
                .bg(if active {
                    p.accent.gpui()
                } else {
                    p.border.gpui()
                })
        }))
}

fn appearance_step(app: &CleanApp, cx: &mut Context<CleanApp>) -> impl IntoElement {
    let model = &app.model;
    let p = &model.palette;
    let mode = model.config.appearance.mode;

    div()
        .flex()
        .flex_col()
        .gap(px(18.0))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(9.0))
                .child(section_label("APPEARANCE", p.faint))
                .child(
                    div()
                        .flex()
                        .gap(px(10.0))
                        .children(ThemeMode::ALL.iter().map(|candidate| {
                            theme_card(app, *candidate, mode == *candidate).on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.set_mode(*candidate, cx);
                                },
                            ))
                        })),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .child(section_label("ACCENT", p.faint))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(9.0))
                        .children(Accent::SWATCHES.iter().map(|candidate| {
                            accent_swatch(app, *candidate).on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.set_accent(*candidate, cx);
                                },
                            ))
                        })),
                ),
        )
        .child(
            primary_button("setup-continue", "Continue", p).on_click(cx.listener(
                |this, _, _, cx| {
                    this.model.setup_step = SetupStep::Account;
                    this.model.focused = None;
                    cx.notify();
                },
            )),
        )
}

fn theme_card(app: &CleanApp, mode: ThemeMode, selected: bool) -> Stateful<Div> {
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
            format!("theme-card-{}", mode.label()).into(),
        ))
        .flex()
        .flex_col()
        .gap(px(8.0))
        .flex_1()
        .p(px(9.0))
        .rounded(px(11.0))
        .bg(p.surface.gpui())
        .border_1()
        .border_color(if selected {
            p.accent.gpui()
        } else {
            p.border.with_alpha(0.7).gpui()
        })
        .cursor_pointer()
        .hover(|style| style.border_color(p.accent.with_alpha(0.45).gpui()))
        .child(
            div()
                .flex()
                .flex_col()
                .h(px(64.0))
                .w_full()
                .rounded(px(6.0))
                .overflow_hidden()
                .bg(preview.canvas.gpui())
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(3.0))
                        .h(px(12.0))
                        .px(px(6.0))
                        .bg(preview.sidebar.gpui())
                        .children((0..3).map(|_| {
                            div()
                                .size(px(2.5))
                                .rounded_full()
                                .bg(preview.faint.gpui())
                        })),
                )
                .child(
                    div()
                        .flex()
                        .flex_1()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(4.0))
                                .w(px(26.0))
                                .p(px(5.0))
                                .bg(preview.sidebar.gpui())
                                .children((0..3).map(|_| {
                                    div()
                                        .h(px(3.0))
                                        .w_full()
                                        .rounded_full()
                                        .bg(preview.faint.with_alpha(0.4).gpui())
                                })),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(4.0))
                                .flex_1()
                                .p(px(6.0))
                                .children((0..3).map(|index| {
                                    div()
                                        .h(px(4.0))
                                        .w(px(if index == 0 { 34.0 } else { 46.0 }))
                                        .rounded_full()
                                        .bg(if index == 0 {
                                            preview.accent.gpui()
                                        } else {
                                            preview.faint.with_alpha(0.35).gpui()
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
                .child(
                    div()
                        .text_size(px(11.5))
                        .font_weight(if selected {
                            FontWeight::SEMIBOLD
                        } else {
                            FontWeight::MEDIUM
                        })
                        .text_color(if selected { p.text } else { p.muted }.gpui())
                        .child(mode.label()),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(px(14.0))
                        .rounded_full()
                        .when(selected, |this| this.bg(p.accent.gpui()))
                        .when(!selected, |this| {
                            this.border_1().border_color(p.border.gpui())
                        }),
                ),
        )
}

fn accent_swatch(app: &CleanApp, accent: Accent) -> Stateful<Div> {
    let model = &app.model;
    let p = &model.palette;
    let selected = model.accent() == accent;
    let color = accent.resolve(
        model.config.appearance.mode,
        model.omarchy.as_ref().map(|colors| colors.blue()),
    );

    div()
        .id(ElementId::Name(
            format!("accent-{}", accent.label()).into(),
        ))
        .flex()
        .items_center()
        .justify_center()
        .size(px(30.0))
        .rounded_full()
        .cursor_pointer()
        .border_1()
        .border_color(if selected {
            p.text.with_alpha(0.9).gpui()
        } else {
            Rgba::new(0, 0, 0, 0).gpui()
        })
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .size(px(20.0))
                .rounded_full()
                .bg(color.gpui())
                .when(selected, |this| {
                    this.child(icon(IconName::Check, 10.0, color.best_ink()))
                }),
        )
}

fn account_step(app: &CleanApp, cx: &mut Context<CleanApp>) -> impl IntoElement {
    let model = &app.model;
    let p = &model.palette;
    let provider = model.sign_in.provider.unwrap_or(Provider::Google);
    let saved = model
        .config
        .accounts
        .iter()
        .find(|account| matches!(account.provider, Provider::Apple | Provider::Proton));

    div()
        .flex()
        .flex_col()
        .gap(px(16.0))
        .when_some(saved.cloned(), |this, account| {
            this.child(
                div()
                    .id("saved-account")
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .p(px(11.0))
                    .rounded(px(9.0))
                    .bg(p.accent.with_alpha(if p.is_dark { 0.12 } else { 0.08 }).gpui())
                    .border_1()
                    .border_color(p.accent.with_alpha(0.4).gpui())
                    .cursor_pointer()
                    .hover(|style| style.bg(p.accent.with_alpha(0.2).gpui()))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.use_saved_account(cx);
                    }))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(px(28.0))
                            .rounded_full()
                            .bg(p.accent.with_alpha(0.18).gpui())
                            .child(icon(IconName::CircleCheck, 15.0, p.accent)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(1.0))
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(p.text.gpui())
                                    .child(format!("Continue as {}", account.email)),
                            )
                            .child(
                                div()
                                    .text_size(px(10.5))
                                    .text_color(p.muted.gpui())
                                    .child(format!("Saved {} account · one click", account.provider.label())),
                            ),
                    ),
            )
        })
        .child(
            div()
                .flex()
                .gap(px(8.0))
                .children(
                    [Provider::Google, Provider::Apple, Provider::Proton]
                        .into_iter()
                        .map(|candidate| provider_card(app, candidate, candidate == provider).on_click(
                            cx.listener(move |this, _, _, cx| {
                                this.model.sign_in.provider = Some(candidate);
                                this.model.sign_in.error = None;
                                this.model.focused = None;
                                cx.notify();
                            }),
                        )),
                ),
        )
        .child(match provider {
            Provider::Google => google_fields(app, cx).into_any_element(),
            Provider::Apple => password_fields(app, cx, true).into_any_element(),
            Provider::Proton => password_fields(app, cx, false).into_any_element(),
        })
        .when_some(model.sign_in.error.clone(), |this, error| {
            this.child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(8.0))
                    .p(px(10.0))
                    .rounded(px(8.0))
                    .bg(p.danger.with_alpha(0.10).gpui())
                    .text_size(px(11.5))
                    .line_height(px(16.0))
                    .text_color(p.danger.gpui())
                    .child(error),
            )
        })
        .child(connect_row(app, provider, cx))
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .pt(px(2.0))
                .child(
                    div()
                        .id("setup-skip")
                        .px(px(10.0))
                        .py(px(5.0))
                        .rounded(px(6.0))
                        .cursor_pointer()
                        .text_size(px(11.5))
                        .text_color(p.faint.gpui())
                        .hover(|style| style.text_color(p.muted.gpui()))
                        .on_click(cx.listener(|this, _, _, cx| this.finish_setup(cx)))
                        .child("Skip for now — explore with demo mail"),
                ),
        )
}

fn provider_card(app: &CleanApp, provider: Provider, selected: bool) -> Stateful<Div> {
    let model = &app.model;
    let p = &model.palette;

    let content: AnyElement = match provider {
        Provider::Google => div()
            .flex()
            .items_center()
            .justify_center()
            .size(px(16.0))
            .child(
                svg()
                    .data(include_bytes!("../../assets/google-g.svg"))
                    .text_color(p.text.gpui())
                    .w_full()
                    .h_full(),
            )
            .into_any_element(),
        Provider::Apple => icon(IconName::Apple, 14.0, if selected { p.text } else { p.muted })
            .into_any_element(),
        Provider::Proton => icon(IconName::Shield, 14.0, if selected { p.accent } else { p.muted })
            .into_any_element(),
    };

    div()
        .id(ElementId::Name(
            format!("provider-{}", provider.label()).into(),
        ))
        .flex()
        .flex_1()
        .items_center()
        .justify_center()
        .gap(px(7.0))
        .h(px(42.0))
        .rounded(px(9.0))
        .bg(if selected {
            p.accent.with_alpha(if p.is_dark { 0.16 } else { 0.10 }).gpui()
        } else {
            p.surface.gpui()
        })
        .border_1()
        .border_color(if selected {
            p.accent.with_alpha(0.7).gpui()
        } else {
            p.border.with_alpha(0.7).gpui()
        })
        .cursor_pointer()
        .hover(|style| style.border_color(p.accent.with_alpha(0.4).gpui()))
        .child(content)
        .child(
            div()
                .text_size(px(12.0))
                .font_weight(if selected {
                    FontWeight::SEMIBOLD
                } else {
                    FontWeight::MEDIUM
                })
                .text_color(if selected { p.text } else { p.muted }.gpui())
                .child(provider.label()),
        )
}

fn google_fields(_app: &CleanApp, _cx: &mut Context<CleanApp>) -> impl IntoElement {
    div()
}

fn password_fields(
    app: &CleanApp,
    cx: &mut Context<CleanApp>,
    apple: bool,
) -> impl IntoElement {
    let model = &app.model;
    let p = &model.palette;

    div()
        .flex()
        .flex_col()
        .gap(px(12.0))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(labelled(
                    if apple {
                        "iCloud email"
                    } else {
                        "Proton email"
                    },
                    p,
                ))
                .child(text_field(
                    app,
                    "signin-email",
                    Field::Email,
                    &model.sign_in.email,
                    if apple {
                        "you@icloud.com"
                    } else {
                        "you@proton.me"
                    },
                    false,
                    cx,
                )),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(labelled(
                    if apple {
                        "App-specific password"
                    } else {
                        "Bridge password"
                    },
                    p,
                ))
                .child(text_field(
                    app,
                    "signin-password",
                    Field::Password,
                    &model.sign_in.password,
                    "•••• •••• •••• ••••",
                    true,
                    cx,
                )),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(5.0))
                .id(if apple { "apple-help-link" } else { "proton-help-link" })
                .cursor_pointer()
                .hover(|style| style.text_color(p.accent.gpui()))
                .on_click(cx.listener(move |_, _, _, _| {
                    let url = if apple {
                        "https://appleid.apple.com/account/manage"
                    } else {
                        "https://proton.me/mail/bridge"
                    };
                    crate::mail::render::open_in_browser(url);
                }))
                .child(
                    text(
                        if apple {
                            "Open Apple ID → Sign-In and Security → App-Specific Passwords"
                        } else {
                            "Start Proton Mail Bridge, then copy the password from Mailbox details"
                        },
                        11.0,
                        FontWeight::NORMAL,
                        p.faint,
                    )
                    .line_height(px(16.0)),
                )
                .when(apple, |this| this.child(icon(IconName::ArrowUpRight, 11.0, p.faint))),
        )
}

fn labelled(label: &'static str, p: &Palette) -> impl IntoElement {
    div()
        .text_size(px(11.0))
        .font_weight(FontWeight::MEDIUM)
        .text_color(p.muted.gpui())
        .child(label)
}

fn text_field(
    app: &CleanApp,
    id: &'static str,
    field: Field,
    value: &str,
    placeholder: &str,
    password: bool,
    cx: &mut Context<CleanApp>,
) -> impl IntoElement {
    let p = &app.model.palette;
    let focused = app.model.focused == Some(field);

    field_shell(p, focused)
        .id(id)
        .cursor_text()
        .on_click(cx.listener(move |this, _, window, cx| {
            this.focus_field(field, window, cx);
        }))
        .child(field_value(value, placeholder, focused, password, p))
}

fn connect_row(app: &CleanApp, provider: Provider, cx: &mut Context<CleanApp>) -> impl IntoElement {
    let model = &app.model;
    let p = &model.palette;
    let busy = model.sign_in.busy;

    let label = if busy {
        "Connecting…".to_string()
    } else if provider == Provider::Google {
        "Continue with Google".to_string()
    } else {
        format!("Connect {}", provider.label())
    };

    let button = primary_button("setup-connect", label, p);
    let button = if busy {
        button.opacity(0.65)
    } else {
        button.on_click(cx.listener(|this, _, _, cx| this.start_sign_in(cx)))
    };

    div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(button.when(busy, |this| {
            this.child(icon(IconName::Loader, 13.0, p.accent_ink))
        }))
        .when(busy, |this| {
            this.child(
                div()
                    .id("setup-cancel")
                    .flex()
                    .items_center()
                    .justify_center()
                    .h(px(28.0))
                    .cursor_pointer()
                    .text_size(px(11.5))
                    .text_color(p.muted.gpui())
                    .hover(|style| style.text_color(p.danger.gpui()))
                    .on_click(cx.listener(|this, _, _, cx| this.cancel_sign_in(cx)))
                    .child("Cancel sign in"),
            )
        })
}

fn ready_step(app: &CleanApp, cx: &mut Context<CleanApp>) -> impl IntoElement {
    let model = &app.model;
    let p = &model.palette;

    div()
        .flex()
        .flex_col()
        .gap(px(16.0))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(11.0))
                .p(px(14.0))
                .rounded(px(11.0))
                .bg(p.surface.gpui())
                .border_1()
                .border_color(p.border.with_alpha(0.7).gpui())
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(px(30.0))
                        .rounded_full()
                        .bg(p.success.with_alpha(0.16).gpui())
                        .child(icon(IconName::CircleCheck, 17.0, p.success)),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(2.0))
                        .child(text(
                            "Connected",
                            12.5,
                            FontWeight::SEMIBOLD,
                            p.text,
                        ))
                        .child(text(
                            model
                                .account()
                                .map(|account| account.email.clone())
                                .unwrap_or_else(|| "Demo mail".into()),
                            11.5,
                            FontWeight::NORMAL,
                            p.muted,
                        )),
                ),
        )
        .child(
            primary_button("setup-open", "Open clean", p)
                .on_click(cx.listener(|this, _, _, cx| this.finish_setup(cx))),
        )
}

fn footer(app: &CleanApp, cx: &mut Context<CleanApp>) -> impl IntoElement {
    let model = &app.model;
    let p = &model.palette;
    let back = model.setup_step == SetupStep::Account
        || (model.setup_step == SetupStep::Ready && model.config.accounts.is_empty());

    div()
        .flex()
        .items_center()
        .justify_between()
        .pt(px(4.0))
        .child(if back {
            div()
                .id("setup-back")
                .flex()
                .items_center()
                .gap(px(5.0))
                .cursor_pointer()
                .text_size(px(11.5))
                .text_color(p.muted.gpui())
                .hover(|style| style.text_color(p.text.gpui()))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.model.setup_step = match this.model.setup_step {
                        SetupStep::Account => SetupStep::Appearance,
                        SetupStep::Ready => SetupStep::Account,
                        SetupStep::Appearance => SetupStep::Appearance,
                    };
                    this.model.focused = None;
                    cx.notify();
                }))
                .child(icon(IconName::ArrowLeft, 12.0, p.muted))
                .child("Back")
                .into_any_element()
        } else {
            div().into_any_element()
        })
        .child(
            div()
                .text_size(px(10.5))
                .text_color(p.faint.gpui())
                .child("clean · v0.1"),
        )
}

/// `ThemeMode` helpers used by setup only.
impl ThemeMode {
    pub const ALL: [ThemeMode; 3] = [Self::White, Self::Black, Self::Omarchy];
}

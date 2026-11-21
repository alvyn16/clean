use gpui_kit::*;

use crate::config::Provider;
use crate::theme::Rgba;
use crate::ui::model::Field;
use crate::ui::widgets::*;
use crate::ui::CleanApp;

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
        .bg(Rgba::hex(0x000000)
            .with_alpha(if p.is_dark { 0.45 } else { 0.22 })
            .gpui())
        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
            this.close_account_manager(cx);
        }))
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
                .on_mouse_down(MouseButton::Left, cx.listener(|_, _, _, cx| cx.stop_propagation()))
                .child(header(app, cx))
                .child(
                    div()
                        .flex()
                        .flex_1()
                        .min_h(px(0.0))
                        .child(connected(app, cx))
                        .child(add_new(app, cx)),
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
                        .child("Accounts"),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(p.faint.gpui())
                        .child("manage and connect"),
                ),
        )
        .child(
            icon_button("account-manager-close", IconName::X, p.muted, p.hover, 14.0).on_click(
                cx.listener(|this, _, _, cx| this.close_account_manager(cx)),
            ),
        )
}

fn connected(app: &CleanApp, cx: &mut Context<CleanApp>) -> impl IntoElement {
    let model = &app.model;
    let p = &model.palette;

    div()
        .flex()
        .flex_col()
        .w(px(360.0))
        .flex_shrink_0()
        .border_r(px(1.0))
        .border_color(p.border.with_alpha(0.5).gpui())
        .child(
            div()
                .px(px(18.0))
                .pt(px(18.0))
                .pb(px(10.0))
                .child(section_label("CONNECTED ACCOUNTS", p.faint)),
        )
        .child(
            div()
                .id("manager-accounts-scroll")
                .flex()
                .flex_col()
                .flex_1()
                .overflow_y_scroll()
                .px(px(14.0))
                .gap(px(8.0))
                .children(model.config.accounts.iter().map(|account| {
                    let id = account.id.clone();
                    let active = model.account().map(|a| a.id.as_str()) == Some(account.id.as_str());
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.0))
                        .p(px(11.0))
                        .rounded(px(10.0))
                        .bg(if active {
                            p.selection.gpui()
                        } else {
                            p.surface.gpui()
                        })
                        .border_1()
                        .border_color(p.border.with_alpha(0.6).gpui())
                        .child(account_avatar(app, account))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .flex_1()
                                .min_w(px(0.0))
                                .gap(px(2.0))
                                .child(ellipsis(account.email.clone(), 12.0, FontWeight::MEDIUM, p.text))
                                .child(
                                    div()
                                        .text_size(px(10.5))
                                        .text_color(p.faint.gpui())
                                        .child(account.provider.label()),
                                ),
                        )
                        .child(
                            div()
                                .id(ElementId::Name(format!("manager-disconnect-{}", id).into()))
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
                                        .bg(p.danger.with_alpha(0.12).gpui())
                                        .text_color(p.danger.gpui())
                                })
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.disconnect_account(id.clone(), cx);
                                }))
                                .child("Disconnect"),
                        )
                }))
                .when(model.config.accounts.is_empty(), |this| {
                    this.child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .gap(px(8.0))
                            .h(px(200.0))
                            .child(icon(IconName::Inbox, 24.0, p.faint))
                            .child(text("No accounts connected", 12.0, FontWeight::MEDIUM, p.muted)),
                    )
                }),
        )
}

fn add_new(app: &CleanApp, cx: &mut Context<CleanApp>) -> impl IntoElement {
    let model = &app.model;
    let p = &model.palette;
    let provider = model.sign_in.provider.unwrap_or(Provider::Google);

    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w(px(0.0))
        .px(px(20.0))
        .py(px(18.0))
        .gap(px(14.0))
        .child(section_label("ADD AN ACCOUNT", p.faint))
        .child({
            let cards: Vec<_> = [Provider::Google, Provider::Apple, Provider::Proton]
                .into_iter()
                .map(|candidate| provider_card(app, candidate, candidate == provider, cx))
                .collect();
            div().flex().gap(px(8.0)).children(cards)
        })
        .child(match provider {
            Provider::Google => google_panel(app, cx).into_any_element(),
            Provider::Apple | Provider::Proton => credentials_panel(app, cx).into_any_element(),
        })
        .when_some(model.sign_in.error.clone(), |this, error| {
            let danger = p.danger;
            this.child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(8.0))
                    .p(px(10.0))
                    .rounded(px(8.0))
                    .bg(danger.with_alpha(0.10).gpui())
                    .text_size(px(11.5))
                    .line_height(px(16.0))
                    .text_color(danger.gpui())
                    .child(error),
            )
        })
}

fn provider_card(
    app: &CleanApp,
    provider: Provider,
    selected: bool,
    cx: &mut Context<CleanApp>,
) -> Stateful<Div> {
    let p = &app.model.palette;
    let icon_name = match provider {
        Provider::Google => IconName::Globe,
        Provider::Apple => IconName::Apple,
        Provider::Proton => IconName::Shield,
    };

    div()
        .id(ElementId::Name(format!("manager-provider-{}", provider.label()).into()))
        .flex()
        .flex_1()
        .items_center()
        .justify_center()
        .gap(px(7.0))
        .h(px(40.0))
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
        .on_click(cx.listener(move |this, _, _, cx| this.choose_provider(provider, cx)))
        .child(icon(icon_name, 14.0, if selected { p.accent } else { p.muted }))
        .child(
            div()
                .text_size(px(12.0))
                .font_weight(if selected { FontWeight::SEMIBOLD } else { FontWeight::MEDIUM })
                .text_color(if selected { p.text } else { p.muted }.gpui())
                .child(provider.label()),
        )
}

fn google_panel(app: &CleanApp, cx: &mut Context<CleanApp>) -> impl IntoElement {
    let p = &app.model.palette;
    let busy = app.model.sign_in.busy;

    div()
        .flex()
        .flex_col()
        .gap(px(10.0))
        .child(
            div()
                .id("manager-connect-google")
                .flex()
                .items_center()
                .justify_center()
                .h(px(40.0))
                .rounded(px(9.0))
                .bg(p.accent.gpui())
                .text_size(px(12.5))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(p.accent_ink.gpui())
                .cursor_pointer()
                .shadow_sm()
                .hover(|style| style.opacity(0.92))
                .when(busy, |this| this.opacity(0.65))
                .on_click(cx.listener(|this, _, _, cx| this.start_sign_in(cx)))
                .child(if busy { "Connecting…" } else { "Continue with Google" }),
        )
        .when(busy, |this| {
            this.child(
                div()
                    .id("manager-cancel")
                    .flex()
                    .items_center()
                    .justify_center()
                    .h(px(26.0))
                    .cursor_pointer()
                    .text_size(px(11.5))
                    .text_color(p.muted.gpui())
                    .hover(|style| style.text_color(p.danger.gpui()))
                    .on_click(cx.listener(|this, _, _, cx| this.cancel_sign_in(cx)))
                    .child("Cancel sign in"),
            )
        })
}

fn credentials_panel(app: &CleanApp, cx: &mut Context<CleanApp>) -> impl IntoElement {
    let model = &app.model;
    let p = &model.palette;
    let apple = model.sign_in.provider == Some(Provider::Apple);
    let busy = model.sign_in.busy;

    div()
        .flex()
        .flex_col()
        .gap(px(12.0))
        .child(field(
            app,
            "manager-email",
            Field::Email,
            if apple { "iCloud email" } else { "Proton email" },
            &model.sign_in.email,
            if apple { "you@icloud.com" } else { "you@proton.me" },
            cx,
        ))
        .child(field(
            app,
            "manager-password",
            Field::Password,
            if apple { "App-specific password" } else { "Bridge password" },
            &model.sign_in.password,
            "•••• •••• •••• ••••",
            cx,
        ))
        .child(
            div()
                .id("manager-connect-provider")
                .flex()
                .items_center()
                .justify_center()
                .h(px(40.0))
                .rounded(px(9.0))
                .bg(p.accent.gpui())
                .text_size(px(12.5))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(p.accent_ink.gpui())
                .cursor_pointer()
                .shadow_sm()
                .hover(|style| style.opacity(0.92))
                .when(busy, |this| this.opacity(0.65))
                .on_click(cx.listener(|this, _, _, cx| this.start_sign_in(cx)))
                .child(if busy {
                    "Connecting…".to_string()
                } else {
                    format!("Connect {}", if apple { "iCloud" } else { "Proton" })
                }),
        )
        .when(busy, |this| {
            this.child(
                div()
                    .id("manager-cancel-credentials")
                    .flex()
                    .items_center()
                    .justify_center()
                    .h(px(26.0))
                    .cursor_pointer()
                    .text_size(px(11.5))
                    .text_color(p.muted.gpui())
                    .hover(|style| style.text_color(p.danger.gpui()))
                    .on_click(cx.listener(|this, _, _, cx| this.cancel_sign_in(cx)))
                    .child("Cancel sign in"),
            )
        })
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
        .gap(px(6.0))
        .child(section_label(label, p.faint))
        .child(
            field_shell(p, focused)
                .id(id)
                .cursor_text()
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.focus_field(target, window, cx);
                }))
                .child(field_value(value, placeholder, focused, target == Field::Password, p)),
        )
}

fn account_avatar(app: &CleanApp, account: &crate::config::AccountConfig) -> impl IntoElement {
    let p = &app.model.palette;
    if let Some(path) = app.avatar_for(&account.id) {
        return img(path)
            .id("manager-avatar")
            .size(px(30.0))
            .rounded_full()
            .object_fit(gpui_kit::ObjectFit::Cover)
            .into_any_element();
    }
    div()
        .flex()
        .items_center()
        .justify_center()
        .size(px(30.0))
        .rounded_full()
        .bg(p.accent.with_alpha(0.16).gpui())
        .child(icon(IconName::Globe, 13.0, p.accent))
        .into_any_element()
}

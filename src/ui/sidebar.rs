use gpui_kit::*;

use crate::theme::palette::{Palette, Rgba};
use crate::ui::model::Model;
use crate::ui::widgets::*;
use crate::ui::CleanApp;

const NAV: [(&str, &str, IconName); 6] = [
    ("inbox", "Inbox", IconName::Inbox),
    ("archive", "All Mail", IconName::Archive),
    ("sent", "Sent", IconName::Send),
    ("drafts", "Drafts", IconName::FilePen),
    ("spam", "Spam", IconName::ShieldAlert),
    ("trash", "Trash", IconName::Trash),
];

pub fn sidebar(app: &CleanApp, cx: &mut Context<CleanApp>) -> impl IntoElement {
    let model = &app.model;
    let p = &model.palette;
    let account = model.account();

    div()
        .flex()
        .flex_col()
        .w(px(248.0))
        .h_full()
        .flex_shrink_0()
        .bg(p.sidebar.gpui())
        .border_r(px(1.0))
        .border_color(p.border.with_alpha(0.6).gpui())
        .child(
            div()
                .id("account-header")
                .flex()
                .items_center()
                .gap(px(10.0))
                .px(px(14.0))
                .py(px(14.0))
                .cursor_pointer()
                .hover(|style| style.bg(p.hover.gpui()))
                .on_click(cx.listener(|this, _, _, cx| this.toggle_account_menu(cx)))
                .child(
                    if let Some(account) = account {
                        if let Some(path) = app.avatar_for(&account.id) {
                            img(path)
                                .id("account-avatar")
                                .size(px(32.0))
                                .rounded_full()
                                .object_fit(gpui_kit::ObjectFit::Cover)
                                .into_any_element()
                        } else {
                            fallback_avatar(p)
                        }
                    } else {
                        fallback_avatar(p)
                    },
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w(px(0.0))
                        .gap(px(1.0))
                        .child(ellipsis(
                            account
                                .map(|account| {
                                    if account.display_name.is_empty() {
                                        account.email.clone()
                                    } else {
                                        account.display_name.clone()
                                    }
                                })
                                .unwrap_or_else(|| "clean".into()),
                            12.5,
                            FontWeight::SEMIBOLD,
                            p.text,
                        ))
                        .child(ellipsis(
                            account
                                .map(|account| account.email.clone())
                                .unwrap_or_else(|| "Mail, without the noise".into()),
                            11.0,
                            FontWeight::NORMAL,
                            p.muted,
                        )),
                )
                .child(icon(IconName::ChevronRight, 13.0, p.faint)),
        )
        .child(div().px(px(14.0)).child(hairline(p.border.with_alpha(0.5))))
        .child(
            div()
                .id("sidebar-scroll")
                .flex()
                .flex_col()
                .flex_1()
                .overflow_y_scroll()
                .px(px(8.0))
                .pt(px(12.0))
                .child(
                    div()
                        .px(px(8.0))
                        .pb(px(6.0))
                        .child(section_label("MAIL", p.faint)),
                )
                .children(NAV.iter().map(|(role, label, icon_name)| {
                    nav_row(model, p, role, label, *icon_name, model.folder == *role).on_click(
                        cx.listener(move |this, _, _, cx| {
                            this.model.select_folder((*role).to_string());
                            cx.notify();
                        }),
                    )
                }))
                .when(!model.labels.is_empty(), |this| {
                    this.child(
                        div()
                            .px(px(8.0))
                            .pt(px(16.0))
                            .pb(px(6.0))
                            .child(section_label("LABELS", p.faint)),
                    )
                    .children(model.labels.iter().map(|label| {
                        let color =
                            crate::ui::model::label_color(&label.name, p.accent, p.is_dark);
                        div()
                            .id(label.name.clone())
                            .flex()
                            .items_center()
                            .gap(px(10.0))
                            .h(px(30.0))
                            .px(px(8.0))
                            .rounded(px(6.0))
                            .cursor_pointer()
                            .hover(|style| style.bg(p.hover.gpui()))
                            .child(dot(color, 7.0))
                            .child(
                                div()
                                    .flex_1()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .text_size(px(12.5))
                                    .text_color(p.muted.gpui())
                                    .child(label.name.clone()),
                            )
                            .when(label.count > 0, |this| {
                                this.child(
                                    div()
                                        .text_size(px(11.0))
                                        .text_color(p.faint.gpui())
                                        .child(label.count.to_string()),
                                )
                            })
                    }))
                })
                .when(model.demo, |this| {
                    this.child(
                        div()
                            .mx(px(8.0))
                            .mt(px(18.0))
                            .px(px(8.0))
                            .py(px(7.0))
                            .rounded(px(6.0))
                            .bg(p.inset.with_alpha(0.55).gpui())
                            .text_size(px(10.5))
                            .text_color(p.faint.gpui())
                            .child("Demo mail · connect an account in Settings"),
                    )
                }),
        )
        .child(
            div()
                .p(px(8.0))
                .child(
                    div()
                        .id("sidebar-settings")
                        .flex()
                        .items_center()
                        .gap(px(10.0))
                        .h(px(32.0))
                        .px(px(8.0))
                        .rounded(px(6.0))
                        .cursor_pointer()
                        .hover(|style| style.bg(p.hover.gpui()))
                        .on_click(cx.listener(|this, _, _, cx| this.open_settings(cx)))
                        .child(icon(IconName::Settings2, 15.0, p.muted))
                        .child(text(
                            "Settings",
                            12.5,
                            FontWeight::NORMAL,
                            p.muted,
                        )),
                ),
        )
}

fn nav_row(
    model: &Model,
    p: &Palette,
    role: &'static str,
    label: &'static str,
    icon_name: IconName,
    active: bool,
) -> Stateful<Div> {
    let count = model.folder_unread(role);
    let fg = if active { p.accent } else { p.text };
    let icon_fg = if active { p.accent } else { p.muted };

    div()
        .id(label)
        .flex()
        .items_center()
        .gap(px(10.0))
        .h(px(32.0))
        .px(px(8.0))
        .rounded(px(7.0))
        .cursor_pointer()
        .when(active, |this| this.bg(p.selection.gpui()))
        .when(!active, |this| {
            this.hover(|style| style.bg(p.hover.gpui()))
        })
        .child(icon(icon_name, 15.0, icon_fg))
        .child(
            div()
                .flex_1()
                .text_size(px(12.5))
                .font_weight(if active {
                    FontWeight::MEDIUM
                } else {
                    FontWeight::NORMAL
                })
                .text_color(fg.gpui())
                .child(label),
        )
        .when(count > 0, |this| {
            this.child(
                div()
                    .text_size(px(11.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(if active { p.accent } else { p.faint }.gpui())
                    .child(count.to_string()),
            )
        })
}

pub fn account_menu(app: &CleanApp, cx: &mut Context<CleanApp>) -> impl IntoElement {
    let p = &app.model.palette;
    let accounts = &app.model.config.accounts;
    let active_id = app.model.account().map(|a| a.id.clone());

    div()
        .absolute()
        .top(px(58.0))
        .left(px(8.0))
        .w(px(232.0))
        .p(px(6.0))
        .flex()
        .flex_col()
        .gap(px(2.0))
        .rounded(px(11.0))
        .bg(if p.is_dark {
            Rgba::hex(0x1C1D20).gpui()
        } else {
            Rgba::hex(0xFFFFFF).gpui()
        })
        .border_1()
        .border_color(p.border.with_alpha(0.9).gpui())
        .shadow_lg()
        .on_mouse_down(MouseButton::Left, cx.listener(|_, _, _, cx| cx.stop_propagation()))
        .children(accounts.iter().map(|account| {
            let id = account.id.clone();
            let active = active_id.as_deref() == Some(id.as_str());
            div()
                .id(ElementId::Name(format!("menu-account-{}", id).into()))
                .flex()
                .items_center()
                .gap(px(9.0))
                .h(px(38.0))
                .px(px(8.0))
                .rounded(px(7.0))
                .cursor_pointer()
                .when(active, |this| this.bg(p.selection.gpui()))
                .when(!active, |this| this.hover(|style| style.bg(p.hover.gpui())))
                .on_click(cx.listener(move |this, _, _, cx| this.switch_account(id.clone(), cx)))
                .child(provider_avatar(app, account))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w(px(0.0))
                        .gap(px(1.0))
                        .child(ellipsis(account.email.clone(), 11.5, FontWeight::MEDIUM, p.text))
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(p.faint.gpui())
                                .child(account.provider.label()),
                        ),
                )
                .when(active, |this| this.child(icon(IconName::Check, 12.0, p.accent)))
        }))
        .child(div().h(px(1.0)).my(px(4.0)).bg(p.border.with_alpha(0.5).gpui()))
        .child(menu_action(
            "menu-add-account",
            IconName::Plus,
            "Add another account",
            p,
            cx,
            |this, cx| this.add_account(cx),
        ))
        .child(menu_action(
            "menu-account-settings",
            IconName::Settings2,
            "Account settings",
            p,
            cx,
            |this, cx| this.open_account_settings(cx),
        ))
}

fn menu_action(
    id: &'static str,
    icon_name: IconName,
    label: &'static str,
    p: &Palette,
    cx: &mut Context<CleanApp>,
    action: impl Fn(&mut CleanApp, &mut Context<CleanApp>) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(9.0))
        .h(px(32.0))
        .px(px(8.0))
        .rounded(px(7.0))
        .cursor_pointer()
        .text_size(px(11.5))
        .text_color(p.muted.gpui())
        .hover(|style| style.bg(p.hover.gpui()).text_color(p.text.gpui()))
        .on_click(cx.listener(move |this, _, _, cx| action(this, cx)))
        .child(icon(icon_name, 14.0, p.muted))
        .child(label)
}

fn fallback_avatar(p: &Palette) -> gpui_kit::AnyElement {
    div()
        .flex()
        .items_center()
        .justify_center()
        .size(px(32.0))
        .rounded_full()
        .bg(p.accent.with_alpha(if p.is_dark { 0.22 } else { 0.13 }).gpui())
        .child(icon(IconName::Leaf, 16.0, p.accent))
        .into_any_element()
}

fn provider_avatar(app: &CleanApp, account: &crate::config::AccountConfig) -> impl IntoElement {
    let p = &app.model.palette;
    if let Some(path) = app.avatar_for(&account.id) {
        return img(path)
            .id(ElementId::Name(format!("menu-avatar-{}", account.id).into()))
            .size(px(26.0))
            .rounded_full()
            .object_fit(gpui_kit::ObjectFit::Cover)
            .into_any_element();
    }
    let icon_name = match account.provider {
        crate::config::Provider::Google => IconName::Globe,
        crate::config::Provider::Apple => IconName::Apple,
        crate::config::Provider::Proton => IconName::Shield,
    };
    div()
        .flex()
        .items_center()
        .justify_center()
        .size(px(26.0))
        .rounded_full()
        .bg(p.accent.with_alpha(0.16).gpui())
        .child(icon(icon_name, 13.0, p.accent))
        .into_any_element()
}

pub fn divider(p: &Palette) -> Div {
    div()
        .w(px(1.0))
        .h_full()
        .bg(p.border.with_alpha(0.5).gpui())
}

pub fn muted_separator(p: &Palette) -> Div {
    div().px(px(14.0)).child(hairline(p.border.with_alpha(0.45)))
}

pub fn _unused(_: Rgba) {}

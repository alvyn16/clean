use gpui_kit::*;

use crate::ui::model::MailView;
use crate::ui::widgets::*;
use crate::ui::{
    account_manager, calendar_rail, calendar_view, event_sheet, list, reader, settings, sidebar,
    topbar, CleanApp,
};

pub fn shell(app: &CleanApp, cx: &mut Context<CleanApp>, width: Pixels) -> impl IntoElement {
    let p = app.model.palette.clone();
    let toast = app.model.toast.clone();
    // On a half-screen window there isn't room for four panes; the calendar
    // rail (and eventually the sidebar) would squeeze the reader to zero
    // width. Drop them progressively and expose the folders in an overlay.
    let compact = width < px(1180.0);
    let narrow = width < px(960.0);
    let list_width = if narrow { px(260.0) } else { px(336.0) };

    div()
        .relative()
        .flex()
        .flex_col()
        .size_full()
        .overflow_hidden()
        .bg(p.canvas.gpui())
        .child(topbar::topbar(app, cx, narrow))
        .child(
            div()
                .flex()
                .flex_1()
                .min_h(px(0.0))
                .h_full()
                .overflow_hidden()
                .when(!narrow, |this| this.child(sidebar::sidebar(app, cx)))
                .child(match app.model.view {
                    MailView::Mail => {
                        let mut panes = div()
                            .flex()
                            .flex_1()
                            .min_w(px(0.0))
                            .child(list::message_list(app, cx, list_width))
                            .child(reader::reader(app, cx));
                        if !compact {
                            panes = panes.child(calendar_rail::rail(app, cx));
                        }
                        panes.into_any_element()
                    }
                    MailView::Calendar => calendar_view::calendar_view(app, cx).into_any_element(),
                }),
        )
        .when(narrow && app.model.show_narrow_sidebar, |this| {
            this.child(
                div()
                    .absolute()
                    .top(px(0.0))
                    .left(px(0.0))
                    .right(px(0.0))
                    .bottom(px(0.0))
                    .id("narrow-sidebar-backdrop")
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                        this.model.show_narrow_sidebar = false;
                        cx.notify();
                    }))
                    .child(
                        div()
                            .absolute()
                            .top(px(0.0))
                            .left(px(0.0))
                            .bottom(px(0.0))
                            .on_mouse_down(MouseButton::Left, cx.listener(|_, _, _, cx| cx.stop_propagation()))
                            .child(sidebar::sidebar(app, cx)),
                    ),
            )
        })
        .when(app.model.show_account_menu, |this| {
            this.child(
                div()
                    .absolute()
                    .top(px(0.0))
                    .left(px(0.0))
                    .right(px(0.0))
                    .bottom(px(0.0))
                    .id("account-menu-backdrop")
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                        this.model.show_account_menu = false;
                        cx.notify();
                    }))
                    .child(
                        div()
                            .absolute()
                            .top(px(58.0))
                            .left(px(8.0))
                            .on_mouse_down(MouseButton::Left, cx.listener(|_, _, _, cx| cx.stop_propagation()))
                            .child(sidebar::account_menu(app, cx)),
                    ),
            )
        })
        .when(app.model.show_message_menu, |this| {
            this.child(
                div()
                    .absolute()
                    .top(px(0.0))
                    .left(px(0.0))
                    .right(px(0.0))
                    .bottom(px(0.0))
                    .id("message-menu-backdrop")
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                        this.model.show_message_menu = false;
                        cx.notify();
                    }))
                    .child(list::message_menu(app, cx)),
            )
        })
        .when(app.model.show_account_manager, |this| {
            this.child(account_manager::sheet(app, cx))
        })
        .when(app.model.show_settings, |this| {
            this.child(settings::sheet(app, cx))
        })
        .when(app.model.show_event_sheet, |this| {
            this.child(event_sheet::sheet(app, cx))
        })
        .when_some(toast, |this, (message, _)| {
            this.child(
                div()
                    .absolute()
                    .left(px(0.0))
                    .right(px(0.0))
                    .bottom(px(22.0))
                    .flex()
                    .justify_center()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .h(px(34.0))
                            .px(px(16.0))
                            .rounded_full()
                            .bg(p.text.gpui())
                            .text_color(p.canvas.gpui())
                            .text_size(px(11.5))
                            .font_weight(FontWeight::MEDIUM)
                            .shadow_md()
                            .with_animation(
                                ("toast", message.len() as u64),
                                Animation::new(std::time::Duration::from_millis(220))
                                    .with_easing(ease_out_quint()),
                                |el, t| el.opacity(t).mb(px((1.0 - t) * 8.0)),
                            )
                            .child(message),
                    ),
            )
        })
}

pub fn chrome_hairline(p: &crate::theme::Palette) -> Div {
    hairline(p.border.with_alpha(0.5))
}

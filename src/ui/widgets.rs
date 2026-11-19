use gpui_kit::component::Icon;
use gpui_kit::component::Sizable;
use gpui_kit::*;

use crate::theme::palette::Rgba;

// Re-export the full Lucide icon set (superset of component IconName)
pub use gpui_kit::assets::IconName;
// Bring FluentBuilder (`.when`, `.when_some`, etc.) into scope for all callers
pub use gpui_kit::prelude::FluentBuilder;

pub fn icon(name: IconName, size: f32, color: Rgba) -> Icon {
    Icon::new(name)
        .text_color(color.gpui())
        .with_size(px(size))
}

pub fn icon_button(
    id: impl Into<ElementId>,
    name: IconName,
    color: Rgba,
    hover: Rgba,
    size: f32,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .size(px(size + 14.0))
        .rounded(px(8.0))
        .cursor_pointer()
        .hover(move |style| style.bg(hover.gpui()))
        .child(icon(name, size, color))
}

pub fn avatar(
    initial: impl Into<SharedString>,
    color: Rgba,
    size: f32,
    font_size: f32,
) -> Div {
    div()
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .size(px(size))
        .rounded_full()
        .bg(color.gpui())
        .text_color(color.best_ink().gpui())
        .text_size(px(font_size))
        .font_weight(FontWeight::SEMIBOLD)
        .child(initial.into())
}

pub fn text(
    value: impl Into<SharedString>,
    size: f32,
    weight: FontWeight,
    color: Rgba,
) -> Div {
    div()
        .text_size(px(size))
        .font_weight(weight)
        .text_color(color.gpui())
        .child(value.into())
}

pub fn ellipsis(value: impl Into<SharedString>, size: f32, weight: FontWeight, color: Rgba) -> Div {
    div()
        .overflow_hidden()
        .whitespace_nowrap()
        .text_ellipsis()
        .text_size(px(size))
        .font_weight(weight)
        .text_color(color.gpui())
        .child(value.into())
}

pub fn dot(color: Rgba, size: f32) -> Div {
    div()
        .flex_shrink_0()
        .size(px(size))
        .rounded_full()
        .bg(color.gpui())
}

pub fn hairline(color: Rgba) -> Div {
    div().h(px(1.0)).w_full().bg(color.gpui())
}

pub fn serif(size: f32, weight: FontWeight, color: Rgba) -> Div {
    div()
        .font_family("Instrument Serif")
        .text_size(px(size))
        .font_weight(weight)
        .text_color(color.gpui())
}

pub fn section_label(text: impl Into<SharedString>, color: Rgba) -> Div {
    div()
        .text_size(px(10.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(color.gpui())
        .child(text.into())
}

pub fn chip(text: impl Into<SharedString>, fg: Rgba, bg: Rgba) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(5.0))
        .h(px(18.0))
        .px(px(6.0))
        .rounded(px(4.0))
        .bg(bg.gpui())
        .text_size(px(10.0))
        .font_weight(FontWeight::MEDIUM)
        .text_color(fg.gpui())
        .child(text.into())
}

pub fn label_chip(name: &str, color: Rgba, text: Rgba, bg: Rgba) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(5.0))
        .h(px(17.0))
        .px(px(5.0))
        .rounded(px(3.0))
        .bg(bg.gpui())
        .text_size(px(10.0))
        .font_weight(FontWeight::MEDIUM)
        .text_color(text.gpui())
        .child(dot(color, 5.0))
        .child(name.to_string())
}

pub fn round_button(
    id: impl Into<ElementId>,
    name: IconName,
    p: &crate::theme::palette::Palette,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .size(px(38.0))
        .rounded_full()
        .bg(p.inset.with_alpha(if p.is_dark { 0.9 } else { 1.0 }).gpui())
        .border_1()
        .border_color(p.border.with_alpha(0.5).gpui())
        .cursor_pointer()
        .hover(|style| style.bg(p.hover.gpui()))
        .child(icon(name, 15.0, p.muted))
}

/// A full-width accent button.
pub fn primary_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    p: &crate::theme::palette::Palette,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .gap(px(8.0))
        .h(px(46.0))
        .w_full()
        .rounded(px(11.0))
        .bg(p.accent.gpui())
        .text_color(p.accent_ink.gpui())
        .text_size(px(14.5))
        .font_weight(FontWeight::SEMIBOLD)
        .cursor_pointer()
        .shadow_sm()
        .hover(|style| style.opacity(0.92))
        .child(label.into())
}

/// An inset text field shell. Text and caret are drawn by the caller.
pub fn field_shell(p: &crate::theme::palette::Palette, focused: bool) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(7.0))
        .h(px(38.0))
        .px(px(11.0))
        .w_full()
        .rounded(px(9.0))
        .bg(p.inset.with_alpha(if p.is_dark { 0.85 } else { 1.0 }).gpui())
        .border_1()
        .border_color(if focused {
            p.accent.with_alpha(0.75).gpui()
        } else {
            p.border.with_alpha(0.6).gpui()
        })
}

/// Draws a field value with a caret, masking passwords.
pub fn field_value(
    value: &str,
    placeholder: &str,
    focused: bool,
    password: bool,
    p: &crate::theme::palette::Palette,
) -> Div {
    let shown = if password {
        "•".repeat(value.chars().count())
    } else {
        value.to_string()
    };

    div()
        .flex()
        .flex_1()
        .min_w(px(0.0))
        .items_center()
        .gap(px(1.0))
        .when(shown.is_empty(), |this| {
            this.child(text(
                placeholder.to_string(),
                12.5,
                FontWeight::NORMAL,
                p.faint,
            ))
        })
        .when(!shown.is_empty(), |this| {
            this.child(
                div()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_size(px(12.5))
                    .text_color(p.text.gpui())
                    .child(shown),
            )
        })
        .when(focused, |this| {
            this.child(div().w(px(1.5)).h(px(16.0)).bg(p.accent.gpui()))
        })
}

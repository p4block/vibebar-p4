use gtk4::prelude::*;
use gtk4::{Button, Label};

pub fn button(label: &str) -> Button {
    let button = Button::builder().label(label).build();
    button.add_css_class("btn");
    button
}

pub fn empty_button() -> Button {
    let button = Button::new();
    button.add_css_class("btn");
    button
}

pub fn label_button(label_text: &str) -> (Button, Label) {
    let label = Label::builder().label(label_text).build();
    label.set_overflow(gtk4::Overflow::Visible);
    label.set_margin_start(1);

    let button = empty_button();
    button.set_overflow(gtk4::Overflow::Visible);
    button.set_child(Some(&label));

    (button, label)
}

pub fn set_button_label(button: &Button, cache: &mut String, text: String) {
    if text != *cache {
        button.set_label(&text);
        *cache = text;
    }
}

pub fn set_label(label: &Label, cache: &mut String, text: String) {
    if text != *cache {
        label.set_label(&text);
        *cache = text;
    }
}

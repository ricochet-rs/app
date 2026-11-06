use gpui_component::Icon;

/// The Phosphor icon called `name`, such as `key` or `arrow-clockwise`.
pub fn ph(name: &str) -> Icon {
    Icon::empty().path(format!("icons/phosphor/{name}.svg"))
}

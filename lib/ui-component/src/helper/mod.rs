pub mod colors;
pub mod dialog_container;
pub mod dropdown;
pub mod fallback;
pub mod format;
pub mod head_board;
pub mod scrollbar;
pub mod window_chrome;

/// GRF texture names for a button, for preloading and sizing. Drawing no longer
/// uses them; they go with the preload plumbing.
pub struct ButtonTextures {
    pub normal: &'static str,
    pub hover: &'static str,
    pub pressed: &'static str,
}

/// GRF texture names for a checkbox, for preloading and sizing. See [`ButtonTextures`].
pub struct CheckboxTextures {
    pub off: &'static str,
    pub on: &'static str,
}

pub const CHECKBOX: CheckboxTextures = CheckboxTextures {
    off: ragnarok_resources::ui::CHECKBOX_0,
    on: ragnarok_resources::ui::CHECKBOX_1,
};

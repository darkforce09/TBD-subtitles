//! The Settings window's content (its tab bar, five tabs and footer) and the models banner under
//! the toolbar.

mod automation_tab;
mod engines_tab;
mod form;
mod general_tab;
mod machine_tab;
mod models_banner;
mod models_tab;
mod settings_window;

pub(crate) use models_banner::models_banner_ui;
pub(crate) use settings_window::settings_window_ui;

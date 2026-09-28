//! The Check Lines view: the list of lines and the line editor with its clip and readings.

mod clip_view;
mod heard_list;
mod line_editor;
mod line_list;
mod line_status;
mod review_view;

pub(crate) use review_view::{Playing, ReviewView, review_view_ui};

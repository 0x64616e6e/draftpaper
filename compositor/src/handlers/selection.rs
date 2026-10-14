//! The primary selection (select to copy, middle-click to paste) and data-control (clipboard managers,
//! wl-copy and wl-paste; both the wlr and the ext protocol). The clipboard itself is wl_data_device
//! (handlers/mod.rs).

use smithay::{
    delegate_data_control, delegate_ext_data_control, delegate_primary_selection,
    wayland::selection::{
        ext_data_control, primary_selection::{PrimarySelectionHandler, PrimarySelectionState}, wlr_data_control,
    },
};

use crate::Draftpaper;

impl PrimarySelectionHandler for Draftpaper {
    fn primary_selection_state(&self) -> &PrimarySelectionState {
        &self.primary_selection_state
    }
}
delegate_primary_selection!(Draftpaper);

impl wlr_data_control::DataControlHandler for Draftpaper {
    fn data_control_state(&self) -> &wlr_data_control::DataControlState {
        &self.wlr_data_control_state
    }
}
delegate_data_control!(Draftpaper);

impl ext_data_control::DataControlHandler for Draftpaper {
    fn data_control_state(&self) -> &ext_data_control::DataControlState {
        &self.ext_data_control_state
    }
}
delegate_ext_data_control!(Draftpaper);

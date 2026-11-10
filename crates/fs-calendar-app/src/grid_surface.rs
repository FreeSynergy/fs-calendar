//! The calendar grid as a screen of the shell.
//!
//! Pattern: Adapter (`Surface` over `CalendarGrid`) + Observer (`GridEvents`) — builds the component at the size of the
//! screen; what the reader chooses is handed to a function of the window (open the event in the admin screen).

use std::sync::Arc;

use fs_components_app::{Element, Surface};
use fs_components_calendar_grid::{CalendarGrid, GridEvents, Slot};

/// A grid on a screen.
pub struct GridSurface(pub CalendarGrid);

impl Surface for GridSurface {
    fn build(&self, width: f32, height: f32) -> Box<dyn Element> {
        Box::new(self.0.build(width, height))
    }

    fn styles(&self) -> String {
        fs_components_widgets::styles()
    }
}

/// Tells `open` the key of the event the reader chose.
pub struct OpenOnChoice<F: Fn(&str) + Send + Sync>(pub F);

impl<F: Fn(&str) + Send + Sync> GridEvents for OpenOnChoice<F> {
    fn select_item(&self, key: &str) {
        (self.0)(key);
    }

    fn select_slot(&self, _slot: Slot) {}
}

/// Boxed for the grid.
pub fn events(open: impl Fn(&str) + Send + Sync + 'static) -> Arc<dyn GridEvents> {
    Arc::new(OpenOnChoice(open))
}

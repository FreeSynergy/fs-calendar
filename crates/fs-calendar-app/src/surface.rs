//! The program as a screen of the shell.
//!
//! Pattern: Adapter — the workbench as a `Surface` of the application shell; the only screen, so the
//! shell draws no navigation.

use std::sync::Arc;

use fs_components_app::{AppError, AppWidget, Element, Screen, Surface};
use fs_components_workbenches::calendar::{CalendarSource, CalendarWidget};

use crate::words::{TITLE, labels};

/// The calendar workbench as a surface.
pub struct CalendarScreen(pub CalendarWidget);

impl Surface for CalendarScreen {
    fn build(&self, width: f32, height: f32) -> Box<dyn Element> {
        Box::new(self.0.build(width, height))
    }

    fn styles(&self) -> String {
        self.0.styles()
    }
}

/// The shell around `widget`.
///
/// # Errors
/// [`AppError`] if the shell cannot be made.
pub fn screen_widget(widget: CalendarWidget) -> Result<AppWidget, AppError> {
    let screen = Screen::new("calendar", TITLE, Arc::new(CalendarScreen(widget)));
    AppWidget::new(TITLE, vec![screen])
}

/// The shell around the workbench over `source`.
///
/// # Errors
/// [`AppError`] if the shell cannot be made.
pub fn app_widget(source: Arc<dyn CalendarSource>) -> Result<AppWidget, AppError> {
    screen_widget(CalendarWidget::new(source, labels()))
}

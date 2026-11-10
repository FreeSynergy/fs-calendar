//! The window: the shell of ADR 0065 with the calendar grid and the generated admin screen of `Event`.
//!
//! Pattern: Facade over the shell — one call builds `AppWidget` with two screens (the grid, the admin frontend of the
//! `Event` description) and the standard menus (Program, View, Help). The entries and their ids are those of
//! `fs_admin_frontend::window`; this function has one screen more than `admin_window` can make, so the menus are built
//! here with the same constants. Choosing an event in the grid opens it in the admin screen.

use std::sync::{Arc, OnceLock};

use fs_admin_frontend::view::{AboutInfo, AdminSurface};
use fs_admin_frontend::window::{ABOUT, HELP, QUIT, VIEW_PREFIX};
use fs_components_app::{AppError, AppWidget, Menu, MenuEntry, Screen};
use fs_components_fields::Say;

use crate::grid_surface::GridSurface;

/// Id of the grid screen.
pub const CALENDAR: &str = "Calendar";
/// Id of the admin screen of events.
pub const EVENT: &str = "Event";

fn menus(say: &dyn Say) -> Vec<Menu> {
    let view = |id: &str, label: &str| MenuEntry::new(format!("{VIEW_PREFIX}{id}"), say.say(label));
    vec![
        Menu::new(
            say.say("menu.program"),
            vec![MenuEntry::new(QUIT, say.say("menu.quit"))],
        ),
        Menu::new(
            say.say("menu.view"),
            vec![view(CALENDAR, "screen.calendar"), view(EVENT, "type.Event")],
        ),
        Menu::new(
            say.say("menu.help"),
            vec![
                MenuEntry::new(HELP, say.say("menu.help")),
                MenuEntry::new(ABOUT, say.say("menu.about")),
            ],
        ),
    ]
}

/// The window. `open` is told of the event the reader chose in the grid; it gets the shell to show the admin screen.
///
/// # Errors
/// [`AppError`] if the shell refuses the screens.
pub fn calendar_window(
    title: &str,
    say: &Arc<dyn Say>,
    grid: GridSurface,
    admin: Arc<AdminSurface>,
    about: AboutInfo,
    help_url: String,
    shell: &Arc<OnceLock<AppWidget>>,
) -> Result<AppWidget, AppError> {
    let screens = vec![
        Screen::new(CALENDAR, say.say("screen.calendar"), Arc::new(grid)),
        Screen::new(
            EVENT,
            say.say("type.Event"),
            Arc::clone(&admin) as Arc<dyn fs_components_app::Surface>,
        ),
    ];
    let app = AppWidget::new(title, screens)?;
    let _ = shell.set(app.clone());
    let (handle, surface) = (app.clone(), admin);
    app.set_menus(menus(&**say), move |id| match id {
        QUIT => {
            // an open form with text keeps the window and says why; nothing else is unsaved
            if surface.has_unsaved() {
                handle.select(EVENT);
                surface.warn_unsaved();
            } else {
                std::process::exit(0);
            }
        }
        HELP => fs_components_widgets::open_external(&help_url),
        ABOUT => {
            handle.select(EVENT);
            surface.set_about(Some(about.clone()));
        }
        other => {
            if let Some(view) = other.strip_prefix(VIEW_PREFIX) {
                handle.select(view);
            }
        }
    })?;
    Ok(app)
}

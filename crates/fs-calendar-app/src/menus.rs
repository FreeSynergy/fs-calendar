//! The menus of the window (ADR 0056): what the workbench's buttons do, also from the title bar.
//!
//! Pattern: Command — an entry's id names one transition of the workbench's state. Only real commands;
//! New is disabled if the source cannot make anything. The handler runs on the interface thread with no
//! lock of the shell held, and takes the workbench's only through its closure.

use fs_components_app::{AppError, AppWidget, Menu, MenuEntry};
use fs_components_workbenches::calendar::{CalendarWidget, ViewRange};

use crate::words::menu;

/// Put the menus of the program onto `app`, acting on `widget`.
///
/// # Errors
/// [`AppError`] if the shell refuses the menus.
pub(crate) fn add_menus(app: &AppWidget, widget: CalendarWidget) -> Result<(), AppError> {
    let new = MenuEntry::new("new", menu::NEW);
    let new = if widget.with_state(|s| s.can_create()) {
        new
    } else {
        new.disabled()
    };
    let menus = vec![
        Menu::new(menu::EVENT, vec![new]),
        Menu::new(
            menu::VIEW,
            vec![
                MenuEntry::new("month", menu::MONTH),
                MenuEntry::new("week", menu::WEEK),
                MenuEntry::new("day", menu::DAY),
                MenuEntry::new("agenda", menu::AGENDA),
            ],
        ),
        Menu::new(
            menu::GO,
            vec![
                MenuEntry::new("today", menu::TODAY),
                MenuEntry::new("previous", menu::PREVIOUS),
                MenuEntry::new("next", menu::NEXT),
            ],
        ),
    ];
    app.set_menus(menus, move |id| {
        widget.with_state(|s| match id {
            "new" => {
                s.begin_create();
            }
            "month" => s.set_view(ViewRange::Month),
            "week" => s.set_view(ViewRange::Week),
            "day" => s.set_view(ViewRange::Day),
            "agenda" => s.set_view(ViewRange::Agenda),
            "today" => s.go_today(),
            "previous" => s.previous(),
            "next" => s.next(),
            _ => {}
        });
        widget.refresh();
    })
}

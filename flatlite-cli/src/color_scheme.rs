use ratatui::prelude::{Color, Style};
use ratatui::style::Modifier;
use Color::{Rgb, Reset, White, Black};

pub struct ColorScheme {
    pub active_tab: Style,
    pub inactive_tab: Style,
    pub status_bar: Style,
    pub sheet_heading_inactive: Style,
    pub sheet_heading_active: Style,
    pub debug_panel: Style,
    pub cell: Style,
    pub cell_null: Style,
    pub cell_aligned: Style,
    pub cell_selected: Style,
    pub cell_editing: Style,
    pub cell_editing_cursor: Style,
    pub autocomplete_search: Style,
    pub autocomplete_search_placeholder: Style,
    pub autocomplete_item: Style,
    pub autocomplete_item_selected: Style,
}

impl ColorScheme {
    fn default_rgb() -> Self {
        ColorScheme {
            active_tab: Style::from((Black, White)).add_modifier(Modifier::BOLD),
            inactive_tab: Style::from((Black, Rgb(150, 150, 150))),
            status_bar: Style::from((Black, White)),
            debug_panel: Style::from((Reset, Rgb(4, 22, 51))),
            sheet_heading_active: Style::from((Black, Rgb(200, 200, 235))).add_modifier(Modifier::BOLD).add_modifier(Modifier::UNDERLINED),
            sheet_heading_inactive: Style::from((Rgb(0, 0, 40), White)).add_modifier(Modifier::UNDERLINED),
            cell: Style::from((Reset, Rgb(0, 0, 10))),
            cell_null: Style::from(Rgb(50, 50, 50)),
            cell_aligned: Style::from((Reset, Rgb(20, 20, 60))),
            cell_selected: Style::from((White, Rgb(100, 100, 200))),
            cell_editing: Style::from((Black, White)),
            cell_editing_cursor: Style::from((Black, Rgb(180, 180, 240))),
            autocomplete_search: Style::from((White, Rgb(80, 80, 80))),
            autocomplete_search_placeholder: Style::from((Rgb(120, 120, 120), Rgb(80, 80, 80))),
            autocomplete_item: Style::from((White, Black)),
            autocomplete_item_selected: Style::from((Black, Rgb(200, 200, 255))),
        }
    }
}

impl Default for ColorScheme {
    fn default() -> Self {
        ColorScheme {
            active_tab: Style::from((Color::Black, Color::White)).add_modifier(Modifier::BOLD),
            inactive_tab: Style::default(),
            status_bar: Style::from((Black, White)),
            debug_panel: Style::from((Reset, Color::DarkGray)),
            sheet_heading_active: Style::from((Black, Color::LightBlue)).add_modifier(Modifier::BOLD).add_modifier(Modifier::UNDERLINED),
            sheet_heading_inactive: Style::from((Black, White)).add_modifier(Modifier::UNDERLINED),
            cell: Style::default(),
            cell_null: Style::from(Color::Gray),
            cell_aligned: Style::default(),
            cell_selected: Style::from((Black, Color::White)),
            cell_editing: Style::from((Black, White)),
            cell_editing_cursor: Style::from((White, Black)),
            autocomplete_search: Style::from((White, Black)),
            autocomplete_search_placeholder: Style::from((White, Black)),
            autocomplete_item: Style::from((White, Black)),
            autocomplete_item_selected: Style::from((Black, White)),
        }
    }
}

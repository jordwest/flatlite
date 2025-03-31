pub mod autocomplete;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::{Style, Widget};
use crate::color_scheme::ColorScheme;
use crate::schema::TableId;

pub struct TabBar<'a> {
    pub tabs: Vec<(TableId, String)>,
    pub selected_id: TableId,
    pub color_scheme: &'a ColorScheme,
}

impl <'a> Widget for TabBar<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut x: u16 = 0;

        let mut i = 0;
        for (id, tab) in self.tabs.iter() {
            let size = tab.len() as u16; // TODO: Unicode
            if x + size > area.width {
                // Can't fit any more tabs
                return
            }

            let style = match id == &self.selected_id {
                true => self.color_scheme.active_tab,
                false => self.color_scheme.inactive_tab,
            };

            if i > 0 {
                // Spacer
                buf.set_string(area.x + x, area.y, " ", Style::default());
                x += 1;
            }
            buf.set_string(area.x + x, area.y, tab, Style::from(style));
            x += size;
            i += 1;
        }
    }
}

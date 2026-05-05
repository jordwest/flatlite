pub mod autocomplete;
pub mod swatch;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::{Style, Widget};
use crate::color_scheme::ColorScheme;

pub struct TabBar<'a, Id: Eq + PartialEq> {
    pub tabs: Vec<(Id, String)>,
    pub selected_id: Id,
    pub color_scheme: &'a ColorScheme,
}

impl <'a, Id: Eq + PartialEq> Widget for TabBar<'a, Id> {
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
            let (end_x, _) = buf.set_stringn(area.x + x, area.y, tab, (area.width - x) as usize, Style::from(style));
            x = end_x;
            i += 1;
        }
    }
}

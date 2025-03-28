use std::cmp::max;
use ratatui::prelude::*;
use ratatui::widgets::Clear;
use crate::color_scheme::ColorScheme;
use crate::model::text::TextInput;

pub struct Autocomplete<'a> {
    pub search: &'a TextInput,
    pub placeholder: &'a str,
    pub color_scheme: &'a ColorScheme,
    pub selected_index: usize,
    pub items: &'a Vec<String>,
}

impl <'a> Widget for Autocomplete<'a> {
    fn render(self, area: Rect, buf: &mut Buffer)
    {
        let layout = Layout::new(Direction::Vertical, [
            Constraint::Length(1),
            Constraint::Ratio(1, 1),
        ]).split(area);

        // Max items this list can display at a time
        let max_visible_items = layout[1].height;

        // Render input
        Clear.render(layout[0], buf);

        let (search_style, text) = match self.search.input.len() {
            0 => (self.color_scheme.autocomplete_search_placeholder, self.placeholder),
            _ => (self.color_scheme.autocomplete_search, self.search.input.as_str()),
        };

        buf.set_string(layout[0].x, layout[0].y, ">", search_style);
        buf.set_stringn(layout[0].x + 2, layout[0].y, text, layout[0].width as usize - 2, search_style);
        buf.set_style(layout[0], search_style);

        // Render list
        let mut i = 0;
        for item in self.items {
            if i >= max_visible_items {
                break;
            }

            let item_area = Rect::new(layout[1].x, layout[1].y + i, layout[1].width, 1);

            let is_selected = (i as usize) == self.selected_index;

            let style = match is_selected {
                true => self.color_scheme.autocomplete_item_selected,
                false => self.color_scheme.autocomplete_item,
            };

            Clear.render(item_area, buf);
            buf.set_style(item_area, style);
            buf.set_stringn(item_area.x, item_area.y, &item, item_area.width as usize, style);

            i += 1;
        }
    }
}

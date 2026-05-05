use ratatui::{buffer::Buffer, layout::Rect, widgets::{Clear, Widget}};
use ratatui::prelude::{Color, Style};

pub struct Swatch {}

impl Widget for Swatch {
    fn render(self, area: Rect, buf: &mut Buffer)
    {
        Clear.render(area, buf);

        let colors = vec![
            (Color::Black, "Black"),
            (Color::White, "White"),
            (Color::Red, "Red"),
            (Color::Green, "Green"),
            (Color::Yellow, "Yellow"),
            (Color::Blue, "Blue"),
            (Color::Magenta, "Magenta"),
            (Color::Cyan, "Cyan"),
            (Color::Gray, "Gray"),
            (Color::DarkGray, "DarkGray"),
            (Color::LightRed, "LightRed"),
            (Color::LightGreen, "LightGreen"),
            (Color::LightYellow, "LightYellow"),
            (Color::LightBlue, "LightBlue"),
            (Color::LightMagenta, "LightMagenta"),
            (Color::LightCyan, "LightCyan"),
        ];

        let mut y = area.y;

        let plain = Style::default();

        for (col, name) in colors {
            buf.set_string(area.x, y, name, plain);

            let background = Style::default().bg(col);
            let foreground = Style::default().fg(col);

            buf.set_string(area.x + 15, y, "Background", background);
            buf.set_string(area.x + 30, y, "Foreground", foreground);

            y += 1;
        }
    }
}

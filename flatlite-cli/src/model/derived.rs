use super::{App, SheetRow};

impl App {
    pub fn selected_row(&self) -> &SheetRow {
        let sheet = self.active_sheet().unwrap();
        &sheet.rows[sheet.view_cursor().row()]
    }
}
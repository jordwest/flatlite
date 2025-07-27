use crate::db;
use crate::db::{rusqlite_value_to_string, OrderByBuilder, WhereBuilder};
use crate::schema::{FieldType, TableId};
use crate::util::Vector2i;
use super::App;

pub struct GroupTab {
    pub value: rusqlite::types::Value,
    pub label: String,
}

impl App {
    /// Return the list of tabs when in grouped mode
    pub fn table_group_tabs(&mut self, table_id: TableId) -> Vec<GroupTab> {
        let table = self.schema.table(table_id);
        let existing_cache = self.sheets_cache.get(&table_id);

        let group_by_field = match existing_cache {
            Some(c) => c.group_by_field,
            None => None,
        };

        let group_tabs = match group_by_field {
            Some(field_id) => {
                let field = self.schema.field(field_id);
                let group_values = db::groups(&mut self.conn, &table.name, &field.name).unwrap();
                let mut groups = Vec::new();

                for group in group_values {
                    let label = match &field.field_type {
                        FieldType::StringField => rusqlite_value_to_string(&group),
                        FieldType::SelectField { options } => {
                            options
                                .iter()
                                .find(|o| o.key == rusqlite_value_to_string(&group) )
                                .map(|o| o.label.clone())
                                .flatten()
                                .unwrap_or(rusqlite_value_to_string(&group))
                        },
                        FieldType::BelongsToField(related_table_id, related_field_id) => {
                            let related_table = self.schema.table(*related_table_id);
                            let related_field = self.schema.field(*related_field_id);
                            db::related_title(&mut self.conn, &related_table.name, &related_field.name, &group, "title").unwrap().unwrap_or(rusqlite_value_to_string(&group))
                        }
                    };

                    groups.push(GroupTab {
                        label,
                        value: group.clone(),
                    })
                }

                groups
            },
            None => Vec::new(),
        };

        group_tabs
    }

    pub fn table_order_clause(&self, reverse: bool) -> OrderByBuilder {
        match reverse {
            false => OrderByBuilder::new().asc("__order"),
            true => OrderByBuilder::new().desc("__order"),
        }
    }

    /// Build a WHERE clause for the current table view and any grouping, filters, etc.
    pub fn table_where_clause(&mut self, table_id: TableId) -> WhereBuilder {
        let group_tabs = self.table_group_tabs(table_id);

        let existing_cache = self.sheets_cache.get(&table_id);
        let (group_selected, group_by_field) = match existing_cache {
            Some(c) => (c.group_selected, c.group_by_field),
            None => (0, None),
        };

        let current_group = match group_by_field {
            Some(id) => {
                let field = self.schema.field(id);
                Some(group_tabs[group_selected].value.clone())
            },
            None => None,
        };

        let mut where_clause = WhereBuilder::new();

        if let Some(fid) = group_by_field {
            let group_field = self.schema.field(fid);
            where_clause = where_clause.and(&format!("{} = ?", group_field.name));
        }

        if let Some(ref v) = current_group {
            where_clause = where_clause.param(v.clone())
        }

        where_clause
    }
}

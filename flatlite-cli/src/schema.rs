use ratatui::layout::Constraint::Percentage;
use crate::dbconfig::{ConfigParseState, FieldId, SelectOption, TableId};

#[derive(Debug)]
struct TableSchema {
    id: TableId,
    name: String,
}

#[derive(Debug)]
struct FieldSchema {
    id: FieldId,
    name: String,
    field_type: FieldType,
}

#[derive(Debug)]
struct Schema {
    tables: Vec<TableSchema>,
    fields: Vec<FieldSchema>,
}

#[derive(Debug)]
enum FieldType {
    StringField,
    SelectField { options: Vec<SelectOption> },
    BelongsToField(TableId, FieldId),
}

impl TryFrom<&ConfigParseState> for Schema {
    type Error = eyre::Error;

    fn try_from(config: &ConfigParseState) -> Result<Self, Self::Error> {
        let tables = config.tables.iter().map(|t| TableSchema { id: t.id, name: t.name.to_string() }).collect();
        let mut fields = Vec::with_capacity(config.fields.len());
        
        for field in &config.fields {
            let field_schema = FieldSchema {
                id: field.id,
                name: field.name.to_string(),
                field_type: match &field.type_name {
                    None => FieldType::StringField,
                    Some(ref s) if s == "string" => FieldType::StringField,
                    Some(ref s) if s == "select" => FieldType::SelectField { options: field.options.clone() },
                    Some(ref s) if s == "belongs_to" => {
                        let Some(ref related_entity) = field.related_entity else { return Err(eyre::eyre!("Missing related_entity field for belongs_to relation"))};
                        let Some(ref related_key) = field.related_key else { return Err(eyre::eyre!("Missing related_key field for belongs_to relation"))};

                        let table_id = config.tables.iter().find(|t| t.name.as_str() == related_entity).ok_or(eyre::eyre!("Table {} not found", related_entity))?.id;
                        let field_id = config.fields.iter().find(|f| f.table_id == table_id && f.name.as_str() == related_key).ok_or(eyre::eyre!("Field {} not found", related_key))?.id;

                        FieldType::BelongsToField(table_id, field_id)
                    },
                    Some(ref s) => return Err(eyre::eyre!("Invalid field type {}", s)),
                }
            };

            fields.push(field_schema);
        }

        Ok(Schema {
            tables,
            fields,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eyre::Result;

    #[test]
    fn test_schema_from_config() -> Result<()> {
        let config = ConfigParseState::parse_from_str(include_str!("./config_example.kdl"))?;
        let schema: Schema = (&config).try_into()?;
        insta::assert_debug_snapshot!(schema);
        Ok(())
    }
}

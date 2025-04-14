use std::path::PathBuf;
use crate::dbconfig::Config;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct TableId(pub usize);

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct FieldId(pub usize);

#[derive(Debug, Clone)]
pub struct TableSchema {
    pub id: TableId,
    pub name: String,
    pub label: Option<String>,
    pub source_file: PathBuf,
    pub id_field: Option<FieldId>,
    pub title_field: Option<FieldId>,
    pub fields: Vec<FieldId>,
}

#[derive(Debug)]
pub struct FieldSchema {
    pub id: FieldId,
    pub table_id: TableId,
    pub name: String,
    pub field_type: FieldType,
}

#[derive(Debug)]
pub struct Schema {
    pub tables: Vec<TableSchema>,
    pub fields: Vec<FieldSchema>,
}

#[derive(Debug)]
pub enum FieldType {
    StringField,
    SelectField { options: Vec<SelectOption> },
    BelongsToField(TableId, FieldId),
}

#[derive(Debug, Clone)]
pub struct SelectOption {
    pub key: String,
    pub label: Option<String>,
}

impl Schema {
    pub fn table(&self, id: TableId) -> &TableSchema {
        &self.tables[id.0]
    }

    pub fn field(&self, id: FieldId) -> &FieldSchema {
        &self.fields[id.0]
    }

    pub fn fields_for_table(&self, id: TableId) -> Vec<&FieldSchema> {
        let table = self.table(id);
        let mut vec: Vec<&FieldSchema> = Vec::with_capacity(table.fields.len());
        for field_id in &table.fields {
            vec.push(self.field(*field_id));
        }
        vec
    }

    pub fn table_by_name(&self, name: &str) -> Option<&TableSchema> {
        self.tables.iter().find(|t| t.name == name)
    }

    pub fn field_by_name(&self, table_id: TableId, name: &str) -> Option<&FieldSchema> {
        self.fields.iter().find(|t| t.name == name && t.table_id == table_id)
    }

    /// Convert a list of field IDs to column names to be used in a SQL query
    pub fn field_query(&self, field_ids: &Vec<FieldId>) -> String {
        let mut fields = Vec::with_capacity(field_ids.len());
        for field_id in field_ids {
            fields.push(self.field(*field_id).name.to_string());
        }
        fields.join(", ")
    }
}

impl TryFrom<&Config> for Schema {
    type Error = eyre::Error;

    fn try_from(config: &Config) -> Result<Self, Self::Error> {
        let mut tables: Vec<TableSchema> = config.tables.iter().map(|t| TableSchema {
            id: t.id,
            name: t.name.to_string(),
            label: t.label.clone(),
            id_field: None,
            title_field: None,
            fields: Vec::new(),
            source_file: t.source_file.clone()
        }).collect();
        let mut fields: Vec<FieldSchema> = Vec::with_capacity(config.fields.len());
        
        for field in &config.fields {
            let field_schema = FieldSchema {
                id: field.id,
                table_id: field.table_id,
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
            for t in &mut tables {
                if t.id == field.table_id {
                    t.fields.push(field.id);

                    // TODO: Get this from the schema file instead of hardcoding
                    if field.name == "id" {
                        t.id_field = Some(field.id);
                    }
                    if field.name == "title" {
                        t.title_field = Some(field.id);
                    }
                }
            };
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
        let config = Config::parse_from_str(include_str!("./config_example.kdl"), &PathBuf::from("./config_example.kdl"))?;
        let schema: Schema = (&config).try_into()?;
        insta::assert_debug_snapshot!(schema);
        Ok(())
    }
}

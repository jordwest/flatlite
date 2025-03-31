use kdl::{KdlDocument, KdlNode};
use eyre::Result;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct TableId(usize);

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct FieldId(usize);

/// Represents the db.kdl file
#[derive(Debug, Default)]
pub struct DbConfig {
    pub schema: DbSchema,
}

impl DbConfig {
    pub fn parse_from_str(string: &str) -> Result<Self> {
        let doc: KdlDocument = string.parse()?;

        let mut config = DbConfig::default();

        for node in doc.nodes() {
            if node.name().repr() == Some("schema") {
                config.schema.read_node(&node)?;
            }
        }

        Ok(config)
    }
}

#[derive(Debug, Default)]
pub struct DbSchema {
    pub tables: Vec<DbTable>,
    pub fields: Vec<DbField>,
}

#[derive(Debug, Clone)]
pub struct DbTable {
    pub id: TableId,
    pub name: String,
    pub label: Option<String>,
    pub files: Vec<String>,
    pub fields: Vec<DbField>,
}

#[derive(Debug)]
pub struct TempTable {
    pub id: TableId,
    pub name: String,
}

#[derive(Debug)]
pub struct TempField {
    pub id: FieldId,
    pub table_id: TableId,
    pub related_entity: Option<String>,
    pub related_key: Option<String>,
    pub type_name: Option<String>,
    pub name: String,
    pub options: Vec<SelectOption>,
}

#[derive(Default, Debug)]
pub struct ConfigParseState {
    pub tables: Vec<TempTable>,
    pub fields: Vec<TempField>,
}

trait ExtractValue {
    fn required_name(&self) -> Result<String>;
    fn required(&self, key: &str) -> Result<String>;
    fn optional(&self, key: &str) -> Result<Option<String>>;
}
impl ExtractValue for KdlNode {
    fn required_name(&self) -> Result<String> {
        Ok(self.get(0).ok_or(eyre::eyre!("Missing required name on node {}", self))?.to_string())
    }
    fn required(&self, key: &str) -> Result<String> {
        Ok(self.get(key).ok_or(eyre::eyre!("Missing required property '{}' on node {}", key, self))?.to_string())
    }
    fn optional(&self, key: &str) -> Result<Option<String>> {
        Ok(self.get(key).map(|v| v.to_string()))
    }
}

impl ConfigParseState {
    pub fn add_table(&mut self, name: String) -> TableId {
        let id = TableId(self.tables.len());
        self.tables.push(TempTable {
            id,
            name,
        });
        id
    }

    pub fn add_field(&mut self, table_id: TableId, name: String, type_name: Option<String>, related_entity: Option<String>, related_key: Option<String>, options: Vec<SelectOption>) -> FieldId {
        let id = FieldId(self.fields.len());
        self.fields.push(TempField {
            id,
            name,
            table_id,
            type_name,
            related_entity,
            related_key,
            options,
        });
        id
    }

    pub fn parse_from_str(content: &str) -> Result<ConfigParseState> {
        let doc: KdlDocument = content.parse()?;
        let mut state = ConfigParseState::default();

        for node in doc.nodes() {
            if node.name().repr() == Some("schema") {
                for schema_child in node.iter_children() {
                    if schema_child.name().repr() == Some("table") {
                        let table_id = state.add_table(schema_child.required_name()?);

                        for table_child in schema_child.iter_children() {
                            if table_child.name().repr() == Some("field") {
                                let mut options = Vec::new();
                                for opt in table_child.iter_children() {
                                    if opt.name().repr() == Some("option") {
                                        options.push(SelectOption {
                                            key: opt.required_name()?,
                                            label: opt.optional("label")?,
                                        })
                                    }
                                }

                                state.add_field(
                                    table_id,
                                    table_child.required_name()?,
                                    table_child.optional("type")?,
                                    table_child.optional("related_entity")?,
                                    table_child.optional("related_key")?,
                                    options
                                );
                            }
                        }
                    }
                }
            }
        }

        Ok(state)
    }
    //
    // pub fn field_by_name(&self, table_name: &str, field_name: &str) -> Result<&TempField> {
    // }
}

impl DbSchema {
    pub fn read_node(&mut self, node: &KdlNode) -> Result<()> {
        for child in node.iter_children() {
            if child.name().repr() == Some("table") {
                let table = self.table_node(&child, self.tables.len())?;
                self.tables.push(table);
            }
        }
        Ok(())
    }
    
    pub fn table_node(&mut self, node: &KdlNode, idx: usize) -> Result<DbTable> {
        let mut table = DbTable {
            id: TableId(idx),
            name: String::new(),
            label: None,
            files: Vec::new(),
            fields: Vec::new(),
        };

        for entry in node.iter() {
            if let Some(key) = entry.name() {
                if key.repr() == Some("label") {
                    table.label = Some(entry.value().to_string());
                }
            } else {
                table.name = entry.value().to_string();
            }
        }

        for child in node.iter_children() {
            if child.name().repr() == Some("field") {
                let field = DbField::from_node(child, self.fields.len())?;
                self.fields.push(field.clone());
                table.fields.push(field);
            }
            if child.name().repr() == Some("file") {
                for entry in child.iter() {
                    table.files.push(entry.value().to_string());
                }
            }
        }

        Ok(table)
    }
}

#[derive(Debug, Clone)]
pub struct DbField {
    pub id: FieldId,
    pub name: String,
    pub label: Option<String>,
    pub field_type: FieldType,
}

impl DbField {
    pub fn from_node(node: &KdlNode, idx: usize) -> Result<DbField> {
        let mut field = DbField {
            id: FieldId(idx),
            name: String::new(),
            label: None,
            field_type: FieldType::StringType,
        };

        for entry in node.iter() {
            if let Some(key) = entry.name() {
                if key.repr() == Some("label") {
                    field.label = Some(entry.value().to_string());
                }
            } else {
                field.name = entry.value().to_string();
            }
        }

        field.field_type = FieldType::from_node(node)?;

        Ok(field)
    }
}

#[derive(Debug, Clone)]
pub enum FieldType {
    StringType,
    SelectType(Vec<SelectOption>),
    BelongsTo(String, String),
}

impl FieldType {
    pub fn from_node(node: &KdlNode) -> Result<FieldType> {
        // Default to string type if no type is specified
        let Some(field_type_id) = node.get("type") else { return Ok(FieldType::StringType) };

        match field_type_id.to_string().as_str() {
            "select" => {
                let options = SelectOption::options_from_node(node)?;
                Ok(FieldType::SelectType(options))
            },
            "belongs_to" => {
                let Some(related_entity) = node.get("related_entity") else { return Err(eyre::eyre!("Missing related_entity on belongs_to")) };
                let Some(related_key) = node.get("related_key") else { return Err(eyre::eyre!("Missing related_key on belongs_to")) };
                Ok(FieldType::BelongsTo(related_entity.to_string(), related_key.to_string()))
            },
            _ => Err(eyre::eyre!("Unknown field type {}", field_type_id)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SelectOption {
    pub key: String,
    pub label: Option<String>,
}

impl SelectOption {
    pub fn options_from_node(node: &KdlNode) -> Result<Vec<SelectOption>> {
        let mut vec = Vec::with_capacity(node.iter_children().count());

        for child in node.iter_children() {
            if child.name().repr() == Some("option") {
                let mut option = SelectOption { key: String::new(), label: None };
                for entry in child.iter() {
                    if let Some(key) = entry.name() {
                        if key.repr() == Some("label") {
                            option.label = Some(entry.value().to_string());
                        }
                    } else {
                        option.key = entry.value().to_string();
                    }
                }
                vec.push(option);
            }
        }

        Ok(vec)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse() {
        insta::assert_debug_snapshot!(ConfigParseState::parse_from_str(include_str!("./config_example.kdl")))
    }
}

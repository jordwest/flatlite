use kdl::{KdlDocument, KdlNode};
use eyre::Result;

#[derive(Debug, Clone, Copy)]
pub struct TableId(usize);

#[derive(Debug, Clone, Copy)]
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

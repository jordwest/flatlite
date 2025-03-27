use kdl::{KdlDocument, KdlNode};
use eyre::Result;

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
    pub test: String,
    pub tables: Vec<DbTable>,
}

impl DbSchema {
    pub fn read_node(&mut self, node: &KdlNode) -> Result<()> {
        for child in node.iter_children() {
            if child.name().repr() == Some("table") {
                let table = DbTable::from_node(&child)?;
                self.tables.push(table);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct DbTable {
    pub name: String,
    pub label: Option<String>,
    pub files: Vec<String>,
    pub fields: Vec<DbField>,
}

impl DbTable {
    pub fn from_node(node: &KdlNode) -> Result<DbTable> {
        let mut table = DbTable::default();

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
                let field = DbField::from_node(child)?;
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

#[derive(Debug)]
pub struct DbField {
    pub name: String,
    pub label: Option<String>,
    pub field_type: FieldType,
}

impl DbField {
    pub fn from_node(node: &KdlNode) -> Result<DbField> {
        let mut field = DbField {
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

        Ok(field)
    }
}

#[derive(Debug)]
pub enum FieldType {
    StringType,
    SelectType(Vec<SelectOption>),
    BelongsTo(String, String),
}

#[derive(Debug)]
pub struct SelectOption {
    key: String,
    label: Option<String>,
}

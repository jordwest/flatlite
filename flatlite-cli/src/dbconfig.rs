use std::path::{Path, PathBuf};
use kdl::{KdlDocument, KdlNode};
use eyre::Result;
use crate::schema::{FieldId, SelectOption, TableId};

#[derive(Debug)]
pub struct ConfigTable {
    pub id: TableId,
    pub name: String,
    pub source_file: PathBuf,
}

#[derive(Debug)]
pub struct ConfigField {
    pub id: FieldId,
    pub table_id: TableId,
    pub related_entity: Option<String>,
    pub related_key: Option<String>,
    pub type_name: Option<String>,
    pub name: String,
    pub options: Vec<SelectOption>,
}

#[derive(Default, Debug)]
pub struct Config {
    pub config_source: PathBuf,
    pub tables: Vec<ConfigTable>,
    pub fields: Vec<ConfigField>,
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

impl Config {
    pub fn add_table(&mut self, name: String, source_file: PathBuf) -> TableId {
        let id = TableId(self.tables.len());
        self.tables.push(ConfigTable {
            id,
            name,
            source_file,
        });
        id
    }

    pub fn add_field(&mut self, table_id: TableId, name: String, type_name: Option<String>, related_entity: Option<String>, related_key: Option<String>, options: Vec<SelectOption>) -> FieldId {
        let id = FieldId(self.fields.len());
        self.fields.push(ConfigField {
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

    pub fn parse_from_str(content: &str, source: &Path) -> Result<Config> {
        let doc: KdlDocument = content.parse()?;
        let mut state = Config::default();
        state.config_source = source.to_path_buf();

        for node in doc.nodes() {
            if node.name().repr() == Some("schema") {
                for schema_child in node.iter_children() {
                    if schema_child.name().repr() == Some("table") {
                        let mut path = state.config_source.clone();
                        path.pop();
                        path.push(schema_child.required("file")?);
                        
                        let table_id = state.add_table(schema_child.required_name()?, path);

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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse() {
        insta::assert_debug_snapshot!(Config::parse_from_str(include_str!("./config_example.kdl"), &PathBuf::from("./config_example.kdl")))
    }
}

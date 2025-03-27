use kdl::{KdlDocument, KdlNode};
use std::io::prelude::*;
use rusqlite::{params_from_iter, Connection, Result};

use nom::{
    bytes::complete::{tag},
    IResult,
    Parser,
};
use nom::branch::alt;
use nom::character::complete::alphanumeric1;

#[derive(Clone)]
struct EntityId {
    entity_type: String,
    entity_id: String,
}

struct State(i32);

// fn ident(input: &str) -> IResult<&str, &str> {
//     take_while(|c: char| c.is_alphanumeric()).parse(input)
// }

impl EntityId {
    fn parse(input: &str) -> IResult<&str, EntityId> {
        let (input, (etype, _, eid)) = (alphanumeric1, tag("."), alphanumeric1).parse(input)?;
        Ok((input, EntityId {
            entity_type: etype.to_string(),
            entity_id: eid.to_string(),
        }))
    }
}

#[derive(Clone)]
enum EntityDefinition {
    WithId(EntityId),
    AutoId(String),
}

impl EntityDefinition {
    fn parse_auto_id(input: &str) -> IResult<&str, EntityDefinition> {
        let (input, etype) = alphanumeric1.parse(input)?;
        Ok((input, EntityDefinition::AutoId(etype.to_string())))
    }
    fn parse_with_id(input: &str) -> IResult<&str, EntityDefinition> {
        let (input, (etype, _, eid)) = (alphanumeric1, tag("."), alphanumeric1).parse(input)?;
        Ok((input, EntityDefinition::WithId(EntityId {
            entity_type: etype.to_string(),
            entity_id: eid.to_string(),
        })))
    }

    fn parse(input: &str) -> IResult<&str, EntityDefinition> {
        let (input, _) = tag(".")(input)?;
        let (input, identifier) = alt((EntityDefinition::parse_with_id, EntityDefinition::parse_auto_id)).parse(input)?;
        Ok((input, identifier))
    }

    fn entity_type(&self) -> &str {
        match self {
            EntityDefinition::WithId(eid) => eid.entity_type.as_str(),
            EntityDefinition::AutoId(etype) => etype.as_str(),
        }
    }
}

fn handle_node(state: &mut State, conn: &Connection, belongs_to: Option<EntityId>, node: &KdlNode) {
    // let values = Vec::new();
    //
    // for p in node.iter() {
    //     println!("{}+ {:?},{}", p.name(), p.value())
    // }

    let node_name = node.name().value();
    let edef = EntityDefinition::parse(node_name);
    match edef {
        Ok((_, edef)) => {
            let mut columns = Vec::new();

            let mut title = String::new();

            let id = match &edef {
                EntityDefinition::WithId(eid) => eid.clone(),
                EntityDefinition::AutoId(etype) => {
                    let id = format!("auto-{}", state.0);
                    state.0 += 1;
                    EntityId {
                        entity_type: etype.clone(),
                        entity_id: id,
                    }
                },
            };

            columns.push(("id".to_string(), id.entity_id.to_string()));

            match belongs_to {
                Some(b) => {
                    columns.push((format!("{}_id", b.entity_type), b.entity_id.clone()));
                }
                None => {}
            }

            for entry in node.iter() {
                if let Some(name) = entry.name() {
                    columns.push((name.to_string(), entry.value().to_string()));
                } else {
                    if title.len() > 0 {
                        title.push(' ');
                    }
                    title.push_str(entry.value().to_string().as_str());
                }
            }

            if title.len() > 0 {
                columns.push(("title".to_string(), title));
            }
            let column_names = columns.iter().map(|c| c.0.clone()).collect::<Vec<String>>().join(",");
            let column_val_placeholders = columns.iter().map(|_| "?").collect::<Vec<&str>>().join(",");
            let column_values = columns.iter().map(|c| c.1.clone()).collect::<Vec<String>>();

            let query = format!("INSERT INTO {} ({}) VALUES ({})", edef.entity_type(), column_names, column_val_placeholders);
            let mut stmt = conn.prepare(&query).unwrap();
            let rows = stmt.execute(params_from_iter(column_values)).unwrap();
            println!("affected {} rows", rows);

            for child in node.iter_children() {
                handle_node(state, conn, Some(id.clone()), &child);
            }
        },
        Err(e) => println!("{:?} '{}'", e, node_name)
    }
}

fn main() -> Result<()> {
    let conn = Connection::open("test.sqlite")?;

    let mut state = State(0);
    // println!("{:?}", alphanumeric1("hello"));

    conn.execute("CREATE TABLE status (id TEXT, title TEXT)", [])?;
    conn.execute("CREATE TABLE task (id TEXT, title TEXT, status_id TEXT)", [])?;
    conn.execute("CREATE TABLE recipe (id TEXT, title TEXT)", [])?;
    conn.execute("CREATE TABLE ingredient (id TEXT, title TEXT, recipe_id TEXT, amount TEXT)", [])?;

    let text = std::fs::read_to_string("../todo.kdl").unwrap();
    let doc: KdlDocument = text.parse().expect("failed to parse KDL");
    for node in doc.nodes() {
        handle_node(&mut state, &conn, None, node);
    }

    Ok(())
}

use std::fs::File;
use eyre::Context;
use rusqlite::{params_from_iter, Connection};
use crate::model::{Entity, Schema};

pub fn ingest_csv_table(conn: &Connection, schema: &mut Schema, table_name: &str, source_file: &str) -> eyre::Result<()> {
    let file = File::open(source_file).wrap_err("Failed to open file")?;
    let mut rdr = csv::Reader::from_reader(file);

    let headers = rdr.headers().wrap_err("Failed to read headers")?.iter().collect::<Vec<_>>();

    let columns = headers.iter().map(|s| s.to_string()).collect::<Vec<_>>().join(", ");
    let column_definitions = headers.iter().map(|h| format!("{} TEXT", h)).collect::<Vec<String>>().join(", ");

    let sql = format!("CREATE TABLE {} (__row INTEGER PRIMARY KEY, {})", table_name, column_definitions);
    conn.execute(&sql, [])?;

    let mut e = Entity::default();
    e.table = table_name.to_string();
    e.columns = headers.iter().map(|s| s.to_string()).collect();
    e.source_file = source_file.to_string();
    schema.entities.push(e);

    for row_result in rdr.records() {
        let row = row_result.wrap_err("Failed to read row")?;

        let placeholders = row.iter().map(|_| "?").collect::<Vec<_>>().join(",");

        let sql = format!("INSERT INTO {} ({}) VALUES ({})", table_name, columns, placeholders);
        // let values = row.iter().collect::<Vec<_>>();

        conn.execute(&sql, params_from_iter(row.iter())).wrap_err("Failed to INSERT row")?;
    }

    Ok(())
}

use std::fs::File;
use std::path::Path;
use eyre::Context;
use rusqlite::{params_from_iter, Connection};
use crate::schema::{FieldSchema, Schema, TableId};

pub fn ingest_csv_table(conn: &mut Connection, schema: &mut Schema, table_id: TableId, source_file: &Path) -> eyre::Result<()> {
    let file = File::open(source_file).wrap_err("Failed to open file")?;
    let mut rdr = csv::Reader::from_reader(file);
    let table = schema.table(table_id);

    let headers = rdr.headers().wrap_err("Failed to read headers")?.iter().collect::<Vec<_>>();

    let column_query = headers.iter().map(|s| s.to_string()).collect::<Vec<_>>().join(", ");
    let column_definitions = headers.iter().map(|h| format!("{} TEXT", h)).collect::<Vec<String>>().join(", ");

    let sql = format!("CREATE TABLE {} (__order INTEGER, {})", table.name, column_definitions);
    conn.execute(&sql, [])?;

    conn.execute(&format!("CREATE INDEX {}_order_idx ON {} (__order)", table.name, table.name), [])?;

    let mut rowid = 0;

    for row_result in rdr.records() {
        let row = row_result.wrap_err("Failed to read row")?;

        let placeholders = row.iter().map(|_| "?").collect::<Vec<_>>().join(",");

        let sql = format!("INSERT INTO {} (__order, {}) VALUES ({}, {})", table.name, column_query, rowid, placeholders);

        conn.execute(&sql, params_from_iter(row.iter())).wrap_err("Failed to INSERT row")?;
        rowid += 1;
    }

    Ok(())
}
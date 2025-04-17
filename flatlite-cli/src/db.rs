use std::fs::File;
use std::path::{Path};
use eyre::Context;
use rusqlite::{params_from_iter, Connection, Params, ParamsFromIter};
use rusqlite::types::{FromSql, Value};
use crate::schema::{Schema, TableId};

#[derive(Clone)]
pub struct WhereBuilder {
    clauses: Vec<String>,
    params: Vec<rusqlite::types::Value>,
}

impl WhereBuilder {
    pub fn new() -> Self {
        WhereBuilder {
            clauses: Vec::new(),
            params: Vec::new(),
        }
    }

    pub fn with(first_clause: &str) -> Self {
        let builder = WhereBuilder::new();
        builder.and(first_clause)
    }

    pub fn and(mut self, clause: &str) -> Self {
        self.clauses.push(match self.clauses.len() {
            0 => format!("({})", clause),
            _ => format!("AND ({})", clause),
        });
        self
    }

    pub fn or(mut self, clause: &str) -> Self {
        self.clauses.push(match self.clauses.len() {
            0 => format!("({})", clause),
            _ => format!("OR ({})", clause),
        });
        self
    }

    pub fn param(mut self, param: rusqlite::types::Value) -> Self {
        self.params.push(param);
        self
    }

    pub fn params(mut self, params: &[rusqlite::types::Value]) -> Self {
        self.params.extend_from_slice(params);
        self
    }

    pub fn as_query(&self) -> String {
        match self.clauses.len() {
            0 => "".to_string(),
            _ => format!("WHERE {}", self.clauses.join(" ")),
        }
    }
}

pub struct QueryBuilder {
    clauses: Vec<String>,
    params: Vec<rusqlite::types::Value>,
}

impl QueryBuilder {
    pub fn with(clause: &str) -> Self {
        QueryBuilder {
            clauses: vec![clause.to_string()],
            params: Vec::new(),
        }
    }

    pub fn add(mut self, clause: &str) -> Self {
        self.clauses.push(clause.to_string());
        self
    }

    pub fn add_where(mut self, where_builder: &WhereBuilder) -> Self {
        self.clauses.push(where_builder.as_query());
        self.params.extend_from_slice(where_builder.params.as_slice());
        self
    }

    pub fn param(mut self, p: rusqlite::types::Value) -> Self {
        self.params.push(p);
        self
    }

    pub fn params(mut self, params: &[rusqlite::types::Value]) -> Self {
        self.params.extend_from_slice(params);
        self
    }

    pub fn params_iter(&self) -> ParamsFromIter<std::slice::Iter<rusqlite::types::Value>> {
        params_from_iter(self.params.iter())
    }

    pub fn as_query(&self) -> String {
        self.clauses.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use crate::db::{QueryBuilder, WhereBuilder};

    #[test]
    fn test_query_builder() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();

        let _ = conn.execute("CREATE TABLE test (thing INT, blah INTEGER)", []).unwrap();
        let _ = conn.execute("INSERT INTO test (thing, blah) VALUES (1, 2)", []).unwrap();
        let _ = conn.execute("INSERT INTO test (thing, blah) VALUES (10, 20)", []).unwrap();

        let builder = QueryBuilder::with("SELECT *, rowid FROM test")
            .add_where(
                &WhereBuilder::with("thing = ?")
                    .param(rusqlite::types::Value::Integer(10))
                    .and("blah = ?")
                    .param(rusqlite::types::Value::Integer(20))
            )
            .add("LIMIT 10");

        assert_eq!(builder.as_query(), "SELECT *, rowid FROM test WHERE (thing = ?) AND (blah = ?) LIMIT 10");

        let mut stmt = conn.prepare(&builder.as_query()).unwrap();
        let result: i32 = stmt.query_row(builder.params_iter(), |row| row.get(0)).unwrap();

        assert_eq!(result, 10);
    }
}

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

    let mut order = 0;

    for row_result in rdr.records() {
        let row = row_result.wrap_err("Failed to read row")?;

        let placeholders = row.iter().map(|_| "?").collect::<Vec<_>>().join(",");

        let sql = format!("INSERT INTO {} (__order, {}) VALUES ({}, {})", table.name, column_query, order, placeholders);

        conn.execute(&sql, params_from_iter(row.iter())).wrap_err("Failed to INSERT row")?;
        order += 1;
    }

    Ok(())
}

pub fn single_result<R: FromSql, P: Params>(conn: &mut Connection, query: &str, params: P) -> eyre::Result<R> {
    let mut count_stmt = conn.prepare(query)?;
    Ok(count_stmt.query_row(params, |r| {
        r.get(0)
    })?)
}

pub fn rusqlite_value_to_string(v: &Value) -> String {
    match v {
        Value::Text(v) => v.clone(),
        Value::Integer(v) => v.to_string(),
        Value::Real(v) => v.to_string(),
        Value::Blob(v) => format!("<{} bytes>", v.len()),
        Value::Null => "<null>".to_string(),
    }
}

pub fn groups(conn: &mut Connection, table_name: &str, field_name: &str) -> eyre::Result<Vec<rusqlite::types::Value>> {
    let mut stmt = conn.prepare(&format!("SELECT {} FROM {} GROUP BY {}", field_name, table_name, field_name))?;
    let mut v: Vec<rusqlite::types::Value> = Vec::new();

    for row in stmt.query_map([], |row| row.get(0))? {
        v.push(row?);
    }

    Ok(v)
}

pub fn related_title(conn: &mut Connection, table_name: &str, key_name: &str, key_value: &Value, title_field: &str) -> eyre::Result<String> {
    single_result(conn, &format!("SELECT {} FROM {} WHERE {} = ?", title_field, table_name, key_name), [key_value])
}
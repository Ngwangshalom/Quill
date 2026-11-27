//! SQLite operations used by Quill's `db` module.
//!
//! Programs pass a database path to each operation. Keeping connections short-lived
//! means a handle cannot outlive its permit or the program that created it.

use std::path::{Path, PathBuf};

use rusqlite::types::Value as SqlValue;
use rusqlite::{params_from_iter, Connection, OpenFlags};
use serde_json::{Map, Number, Value as JsonValue};

/// Opens (and, when needed, creates) a SQLite database at `path`.
pub fn open(folder: &Path, path: &str) -> Result<(), String> {
    let path = resolve(folder, path);
    Connection::open(path)
        .map(|_| ())
        .map_err(|error| format!("SQLite couldn't open the database: {error}"))
}

/// Runs a write statement with named parameters and returns the number of changed rows.
pub fn execute(folder: &Path, path: &str, sql: &str, parameters: &str) -> Result<usize, String> {
    let path = resolve(folder, path);
    let connection = Connection::open(path)
        .map_err(|error| format!("SQLite couldn't open the database: {error}"))?;
    let bindings = bindings(&connection, sql, parameters)?;
    connection
        .execute(sql, params_from_iter(bindings))
        .map_err(|error| format!("SQLite couldn't run this change: {error}"))
}

/// Reads rows as JSON text. A Quill program can read that text as `list of Booking`.
pub fn query(folder: &Path, path: &str, sql: &str, parameters: &str) -> Result<String, String> {
    let path = resolve(folder, path);
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|error| format!("SQLite couldn't open the database for reading: {error}"))?;
    connection
        .pragma_update(None, "query_only", "ON")
        .map_err(|error| format!("SQLite couldn't make this connection read-only: {error}"))?;

    let bindings = bindings(&connection, sql, parameters)?;
    let mut statement = connection
        .prepare(sql)
        .map_err(|error| format!("SQLite couldn't prepare this question: {error}"))?;
    let columns: Vec<String> = statement
        .column_names()
        .iter()
        .map(|name| (*name).to_string())
        .collect();
    let mut rows = statement
        .query(params_from_iter(bindings))
        .map_err(|error| format!("SQLite couldn't read these rows: {error}"))?;
    let mut result = Vec::new();
    while let Some(row) = rows
        .next()
        .map_err(|error| format!("SQLite couldn't read the next row: {error}"))?
    {
        let mut record = Map::new();
        for (index, name) in columns.iter().enumerate() {
            let value = row
                .get::<_, SqlValue>(index)
                .map_err(|error| format!("SQLite couldn't read column {name}: {error}"))?;
            record.insert(name.clone(), json_value(value, name)?);
        }
        result.push(JsonValue::Object(record));
    }
    serde_json::to_string(&JsonValue::Array(result))
        .map_err(|error| format!("Quill couldn't write the database rows as JSON: {error}"))
}

fn resolve(folder: &Path, path: &str) -> PathBuf {
    let path = Path::new(path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        folder.join(path)
    }
}

/// Parameters are a JSON record. Its field `id` binds to SQLite's `:id` placeholder.
fn bindings(connection: &Connection, sql: &str, parameters: &str) -> Result<Vec<SqlValue>, String> {
    let parsed: JsonValue = serde_json::from_str(parameters).map_err(|error| {
        format!("Database parameters need to be JSON written with json(record ...): {error}")
    })?;
    let JsonValue::Object(fields) = parsed else {
        return Err("Database parameters need to be a record, written with json(record ...).".into());
    };
    let count = connection
        .prepare(sql)
        .map_err(|error| format!("SQLite couldn't prepare this statement: {error}"))?
        .parameter_count();
    let mut values = vec![SqlValue::Null; count];
    for (name, value) in fields {
        let placeholder = format!(":{name}");
        let index = connection
            .prepare(sql)
            .map_err(|error| format!("SQLite couldn't prepare this statement: {error}"))?
            .parameter_index(&placeholder)
            .map_err(|error| format!("SQLite couldn't find parameter {placeholder}: {error}"))?
            .ok_or_else(|| format!("This SQL has no parameter named {placeholder}."))?;
        values[index - 1] = sql_value(value, &name)?;
    }
    Ok(values)
}

fn sql_value(value: JsonValue, name: &str) -> Result<SqlValue, String> {
    match value {
        JsonValue::Null => Ok(SqlValue::Null),
        JsonValue::Bool(value) => Ok(SqlValue::Integer(i64::from(value))),
        JsonValue::Number(number) => {
            if let Some(value) = number.as_i64() {
                Ok(SqlValue::Integer(value))
            } else if let Some(value) = number.as_u64().and_then(|value| i64::try_from(value).ok()) {
                Ok(SqlValue::Integer(value))
            } else if let Some(value) = number.as_f64() {
                if value.is_finite() {
                    Ok(SqlValue::Real(value))
                } else {
                    Err(format!("Database parameter {name} isn't a finite number."))
                }
            } else {
                Err(format!("Database parameter {name} is outside SQLite's number range."))
            }
        }
        JsonValue::String(value) => Ok(SqlValue::Text(value)),
        JsonValue::Array(_) | JsonValue::Object(_) => Err(format!(
            "Database parameter {name} is a nested value. SQLite parameters can be text, numbers, true/false, or missing."
        )),
    }
}

fn json_value(value: SqlValue, column: &str) -> Result<JsonValue, String> {
    match value {
        SqlValue::Null => Ok(JsonValue::Null),
        SqlValue::Integer(value) => Ok(JsonValue::Number(Number::from(value))),
        SqlValue::Real(value) => Number::from_f64(value)
            .map(JsonValue::Number)
            .ok_or_else(|| format!("Column {column} contains a number JSON cannot represent.")),
        SqlValue::Text(value) => Ok(JsonValue::String(value)),
        SqlValue::Blob(_) => Err(format!(
            "Column {column} contains binary data. Read it as text or leave binary support for a later version."
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder() -> PathBuf {
        let folder = std::env::temp_dir().join(format!("quill-db-test-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        folder
    }

    #[test]
    fn creates_database_executes_bound_values_and_reads_json_rows() {
        let folder = folder();
        open(&folder, "bookings.sqlite").unwrap();
        execute(
            &folder,
            "bookings.sqlite",
            "create table bookings (id integer, name text)",
            "{}",
        )
        .unwrap();
        execute(
            &folder,
            "bookings.sqlite",
            "insert into bookings values (:id, :name)",
            r#"{"id":7,"name":"Amina"}"#,
        )
        .unwrap();
        assert_eq!(
            query(
                &folder,
                "bookings.sqlite",
                "select id, name from bookings where id = :id",
                r#"{"id":7}"#,
            )
            .unwrap(),
            r#"[{"id":7,"name":"Amina"}]"#
        );
        std::fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn rejects_non_record_parameters() {
        assert!(bindings(&Connection::open_in_memory().unwrap(), "select 1", "[]")
            .unwrap_err()
            .contains("need to be a record"));
    }
}

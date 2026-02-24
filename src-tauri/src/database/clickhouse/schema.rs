use std::collections::{HashMap, HashSet};

use serde::Deserialize;

use crate::{
    database::types::{ColumnInfo, DatabaseSchema, TableInfo},
    Error,
};

use super::connect::ClickHouseClient;

#[derive(Deserialize)]
struct ClickHouseSchemaResponse {
    #[allow(unused)]
    meta: Vec<ColumnMeta>,
    data: Vec<Vec<serde_json::Value>>,
    #[allow(unused)]
    rows: usize,
}

#[derive(Deserialize)]
struct ColumnMeta {
    #[allow(unused)]
    name: String,
    #[serde(rename = "type")]
    #[allow(unused)]
    column_type: String,
}

pub async fn get_database_schema(client: &ClickHouseClient) -> Result<DatabaseSchema, Error> {
    let schema_query = r#"
        SELECT
            database,
            table,
            name,
            type,
            default_kind != '' as has_default,
            default_expression
        FROM system.columns
        WHERE database = currentDatabase()
        ORDER BY database, table, position
        FORMAT JSONCompact
    "#;

    let response_text = client.execute_raw(schema_query).await?;

    let response: ClickHouseSchemaResponse = serde_json::from_str(&response_text).map_err(|e| {
        Error::Any(anyhow::anyhow!(
            "Failed to parse schema response: {}",
            e
        ))
    })?;

    let mut tables_map: HashMap<(&str, &str), TableInfo> = HashMap::new();
    let mut schemas_set = HashSet::new();
    let mut unique_columns_set = HashSet::new();

    // Store string data so we can reference it
    let rows: Vec<Vec<String>> = response
        .data
        .iter()
        .map(|row| {
            row.iter()
                .map(|v| match v {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                })
                .collect()
        })
        .collect();

    for row in &rows {
        if row.len() < 6 {
            continue;
        }

        let database = row[0].as_str();
        let table_name = row[1].as_str();
        let column_name = row[2].as_str();
        let data_type = row[3].as_str();
        let has_default = row[4].as_str();
        let default_expression = row[5].as_str();

        schemas_set.insert(database.to_owned());
        unique_columns_set.insert(column_name.to_owned());

        let is_nullable = data_type.starts_with("Nullable(");
        let default_value = if has_default == "1" || has_default == "true" {
            Some(default_expression.to_owned())
        } else {
            None
        };

        let table_key = (database, table_name);

        let table_info = tables_map.entry(table_key).or_insert_with(|| TableInfo {
            name: table_name.to_owned(),
            schema: database.to_owned(),
            columns: Vec::new(),
        });

        table_info.columns.push(ColumnInfo {
            name: column_name.to_owned(),
            data_type: data_type.to_owned(),
            is_nullable,
            default_value,
        });
    }

    let tables = tables_map.into_values().collect();
    let schemas = schemas_set.into_iter().collect();
    let unique_columns = unique_columns_set.into_iter().collect();

    Ok(DatabaseSchema {
        tables,
        schemas,
        unique_columns,
    })
}

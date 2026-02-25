use std::time::Instant;

use serde::Deserialize;
use serde_json::value::RawValue;

use crate::{
    database::{parser::ParsedStatement, types::ExecSender, QueryExecEvent},
    utils::serialize_as_json_array,
    Error,
};

use super::connect::ClickHouseClient;

/// Default maximum rows returned when the query has no explicit LIMIT clause.
const DEFAULT_LIMIT: usize = 1000;

#[derive(Deserialize)]
struct ClickHouseResponse {
    meta: Vec<ColumnMeta>,
    data: Vec<Vec<serde_json::Value>>,
    #[allow(unused)]
    rows: usize,
}

#[derive(Deserialize)]
struct ColumnMeta {
    name: String,
    #[serde(rename = "type")]
    #[allow(unused)]
    column_type: String,
}

pub async fn execute_query(
    client: &ClickHouseClient,
    stmt: ParsedStatement,
    sender: &ExecSender,
) -> Result<(), Error> {
    if stmt.returns_values {
        execute_query_with_results(client, &stmt.statement, stmt.has_explicit_limit, sender)
            .await?;
    } else {
        execute_modification_query(client, &stmt.statement, sender).await?;
    }
    Ok(())
}

async fn execute_query_with_results(
    client: &ClickHouseClient,
    query: &str,
    has_explicit_limit: bool,
    sender: &ExecSender,
) -> Result<(), Error> {
    let started_at = Instant::now();
    log::info!("Starting ClickHouse query: {}", query);

    // Strip trailing semicolons.
    let trimmed = query.trim().trim_end_matches(';');

    // If no explicit limit, fetch DEFAULT_LIMIT + 1 rows so we can detect truncation.
    let limited_query = if !has_explicit_limit {
        format!("{} LIMIT {}", trimmed, DEFAULT_LIMIT + 1)
    } else {
        trimmed.to_string()
    };

    let query_with_format = format!("{} FORMAT JSONCompact", limited_query);

    let response_text = match client.execute_raw(&query_with_format).await {
        Ok(text) => text,
        Err(e) => {
            let error_msg = format!("Query failed: {}", e);
            sender.send(QueryExecEvent::Finished {
                elapsed_ms: started_at.elapsed().as_millis() as u64,
                affected_rows: 0,
                error: Some(error_msg.clone()),
                truncated: false,
            })?;
            return Err(Error::Any(anyhow::anyhow!(error_msg)));
        }
    };

    let response: ClickHouseResponse = match serde_json::from_str(&response_text) {
        Ok(r) => r,
        Err(e) => {
            let error_msg = format!("Failed to parse ClickHouse response: {}", e);
            sender.send(QueryExecEvent::Finished {
                elapsed_ms: started_at.elapsed().as_millis() as u64,
                affected_rows: 0,
                error: Some(error_msg.clone()),
                truncated: false,
            })?;
            return Err(Error::Any(anyhow::anyhow!(error_msg)));
        }
    };

    // Detect truncation: we fetched DEFAULT_LIMIT + 1 rows, so if we got more than DEFAULT_LIMIT
    // the result was cut off.
    let truncated = !has_explicit_limit && response.data.len() > DEFAULT_LIMIT;
    let data = if truncated {
        &response.data[..DEFAULT_LIMIT]
    } else {
        &response.data[..]
    };

    // Send column info
    let column_names: Vec<&str> = response.meta.iter().map(|m| m.name.as_str()).collect();
    let columns = serialize_as_json_array(column_names.iter().copied())?;
    sender.send(QueryExecEvent::TypesResolved { columns })?;

    // Send rows in batches of 50
    let batch_size = 50;
    let total_rows = data.len();

    for chunk in data.chunks(batch_size) {
        let json = serde_json::to_string(chunk)
            .map_err(|e| Error::Any(anyhow::anyhow!("Failed to serialize rows: {}", e)))?;
        let page = RawValue::from_string(json)
            .map_err(|e| Error::Any(anyhow::anyhow!("Invalid JSON: {}", e)))?;

        sender.send(QueryExecEvent::Page {
            page_amount: chunk.len(),
            page,
        })?;
    }

    let duration = started_at.elapsed().as_millis() as u64;
    sender.send(QueryExecEvent::Finished {
        elapsed_ms: duration,
        affected_rows: 0,
        error: None,
        truncated,
    })?;

    log::info!(
        "ClickHouse query completed: {} rows in {}ms{}",
        total_rows,
        duration,
        if truncated { " (truncated)" } else { "" }
    );

    Ok(())
}

async fn execute_modification_query(
    client: &ClickHouseClient,
    query: &str,
    sender: &ExecSender,
) -> Result<(), Error> {
    log::info!("Executing ClickHouse modification query: {}", query);
    let started_at = Instant::now();

    match client.execute_raw(query).await {
        Ok(_) => {
            sender.send(QueryExecEvent::Finished {
                elapsed_ms: started_at.elapsed().as_millis() as u64,
                affected_rows: 0,
                error: None,
                truncated: false,
            })?;
            Ok(())
        }
        Err(e) => {
            let error_msg = format!("Query failed: {}", e);
            sender.send(QueryExecEvent::Finished {
                elapsed_ms: started_at.elapsed().as_millis() as u64,
                affected_rows: 0,
                error: Some(error_msg.clone()),
                truncated: false,
            })?;
            Err(Error::Any(anyhow::anyhow!(error_msg)))
        }
    }
}

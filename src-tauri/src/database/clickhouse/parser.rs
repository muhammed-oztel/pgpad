use sqlparser::{ast::Statement, dialect::ClickHouseDialect};

use crate::database::{
    self,
    parser::{ParsedStatement, SqlDialectExt},
};

pub fn parse_statements(query: &str) -> anyhow::Result<Vec<ParsedStatement>> {
    database::parser::parse_statements(&ClickHouseDialect {}, query)
}

impl SqlDialectExt for ClickHouseDialect {
    fn returns_values(stmt: &Statement) -> bool {
        match stmt {
            Statement::Query { .. } => true,
            Statement::ShowVariable { .. } => true,
            Statement::ShowColumns { .. } => true,
            Statement::ShowTables { .. } => true,
            Statement::ShowDatabases { .. } => true,
            Statement::ShowCreate { .. } => true,
            Statement::Explain { .. } => true,
            Statement::ExplainTable { .. } => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_select_statements() {
        let results = parse_statements("SELECT * FROM system.tables").unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].returns_values);
    }

    #[test]
    fn parses_create_table() {
        let results = parse_statements(
            "CREATE TABLE test (id UInt32, name String) ENGINE = MergeTree() ORDER BY id",
        )
        .unwrap();
        assert_eq!(results.len(), 1);
        assert!(!results[0].returns_values);
    }

    #[test]
    fn parses_insert() {
        let results =
            parse_statements("INSERT INTO test (id, name) VALUES (1, 'Alice')").unwrap();
        assert_eq!(results.len(), 1);
        assert!(!results[0].returns_values);
    }

    #[test]
    fn parses_show_statements() {
        let results = parse_statements("SHOW TABLES").unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].returns_values);

        let results = parse_statements("SHOW DATABASES").unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].returns_values);

        let results = parse_statements("SHOW CREATE TABLE test").unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].returns_values);
    }

    #[test]
    fn parses_explain() {
        let results = parse_statements("EXPLAIN SELECT 1").unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].returns_values);
    }

    #[test]
    fn parses_multiple_statements() {
        let results = parse_statements("SELECT 1; SELECT 2").unwrap();
        assert_eq!(results.len(), 2);
        assert!(results[0].returns_values);
        assert!(results[1].returns_values);
    }
}

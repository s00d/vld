//! `vld_data::postgres_types` behaviour tests: values go through
//! `ToSql::to_sql` / `FromSql::from_sql` with the wire bytes tokio-postgres
//! sends and receives (binary format).

mod story_columns;

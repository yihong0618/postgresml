use pgml_macros::{custom_derive, custom_methods};
use sqlx::postgres::PgConnectOptions;
use sqlx::postgres::{PgPool, PgPoolOptions};
use std::borrow::Borrow;
use std::str::FromStr;
use std::time::SystemTime;

use crate::collection::*;
use crate::models;
use crate::queries;
use crate::query_builder;

#[cfg(feature = "javascript")]
use crate::languages::javascript::*;

/// A connection to a postgres database
#[derive(custom_derive, Clone, Debug)]
pub struct Database {
    pub pool: PgPool,
}

#[custom_methods(new, create_or_get_collection, does_collection_exist, archive_collection)]
impl Database {
    /// Create a new [Database]
    ///
    ///  # Arguments
    ///
    ///  * `connection_string` - A postgres connection string, e.g. `postgres://user:pass@localhost:5432/db`
    ///
    ///  # Example
    ///  ```
    ///  use pgml::Database;
    ///
    ///  const CONNECTION_STRING: &str = "postgres://postgres@127.0.0.1:5433/pgml_development";
    ///
    ///  async fn example() -> anyhow::Result<()> {
    ///    let db = Database::new(CONNECTION_STRING).await?;
    ///    // Do stuff with the database
    ///    Ok(())
    ///  }
    ///  ```
    pub async fn new(connection_string: &str) -> anyhow::Result<Self> {
        let connection_options = PgConnectOptions::from_str(connection_string)?;
        let connection_options = connection_options.statement_cache_capacity(0);
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect_with(connection_options)
            .await?;
        sqlx::query(queries::CREATE_COLLECTIONS_TABLE)
            .execute(pool.borrow())
            .await?;
        let pool = pool;
        Ok(Self { pool })
    }

    /// Create a new [Collection] or get an existing one
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the [Collection]
    ///
    /// # Example
    ///```
    /// use pgml::Database;
    ///
    /// const CONNECTION_STRING: &str = "postgres://postgres@127.0.0.1:5433/pgml_development";
    ///
    /// async fn example() -> anyhow::Result<()> {
    ///    let db = Database::new(CONNECTION_STRING).await?;
    ///    let collection = db.create_or_get_collection("collection number 1").await?;
    ///    // Do stuff with the collection
    ///    Ok(())
    /// }
    /// ```
    pub async fn create_or_get_collection(&self, name: &str) -> anyhow::Result<Collection> {
        let collection: Option<models::Collection> = sqlx::query_as::<_, models::Collection>(
            "SELECT * FROM pgml.collections WHERE name = $1 AND active = TRUE;",
        )
        .bind(name)
        .fetch_optional(self.pool.borrow())
        .await?;
        match collection {
            Some(c) => Ok(Collection::from_model_and_pool(c, self.pool.clone())),
            None => Ok(Collection::new(name.to_string(), self.pool.clone()).await?),
        }
    }

    /// Check if a [Collection] exists
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the [Collection]
    ///
    /// # Example
    /// ```
    /// use pgml::Database;
    ///
    /// const CONNECTION_STRING: &str = "postgres://postgres@localhost:5432/pgml_development";
    ///
    /// async fn example() -> anyhow::Result<()> {
    ///   let db = Database::new(CONNECTION_STRING).await?;
    ///   let collection_exists = db.does_collection_exist("collection number 1").await?;
    ///   // Do stuff with your new found information
    ///   Ok(())
    /// }
    /// ```
    pub async fn does_collection_exist(&self, name: &str) -> anyhow::Result<bool> {
        let collection: Option<models::Collection> = sqlx::query_as::<_, models::Collection>(
            "SELECT * FROM pgml.collections WHERE name = $1 AND active = TRUE;",
        )
        .bind(name)
        .fetch_optional(self.pool.borrow())
        .await?;
        Ok(collection.is_some())
    }

    /// Archive a [Collection]
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the [Collection]
    ///
    /// # Example
    ///```
    /// use pgml::Database;
    ///
    /// const CONNECTION_STRING: &str = "postgres://postgres@127.0.0.1:5433/pgml_development";
    ///
    /// async fn example() -> anyhow::Result<()> {
    ///    let db = Database::new(CONNECTION_STRING).await?;
    ///    db.archive_collection("collection number 1").await?;
    ///    Ok(())
    /// }
    /// ```
    pub async fn archive_collection(&self, name: &str) -> anyhow::Result<()> {
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("Error getting system time")
            .as_secs();
        let archive_table_name = format!("{}_archive_{}", name, timestamp);
        sqlx::query("UPDATE pgml.collections SET name = $1, active = FALSE where name = $2")
            .bind(&archive_table_name)
            .bind(name)
            .execute(self.pool.borrow())
            .await?;
        sqlx::query(&query_builder!(
            "ALTER SCHEMA %s RENAME TO %s",
            name,
            archive_table_name
        ))
        .execute(self.pool.borrow())
        .await?;
        Ok(())
    }
}

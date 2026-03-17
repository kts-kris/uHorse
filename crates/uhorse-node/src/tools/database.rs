//! 数据库执行器
//!
//! 支持多种数据库的连接和查询操作

use crate::error::{NodeError, NodeResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uhorse_protocol::{CommandOutput, DatabaseCommand, DatabaseType};

/// 数据库连接配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConnection {
    /// 连接名称
    pub name: String,
    /// 数据库类型
    pub db_type: DatabaseType,
    /// 连接字符串
    pub connection_string: String,
    /// 最大连接数
    pub max_connections: u32,
    /// 连接超时
    pub connect_timeout_secs: u64,
    /// 是否只读
    pub read_only: bool,
}

/// 数据库执行器
pub struct DatabaseExecutor {
    /// 预配置的连接
    connections: Arc<RwLock<HashMap<String, DatabaseConnection>>>,
    /// 默认超时
    default_timeout: Duration,
}

impl DatabaseExecutor {
    /// 创建新的数据库执行器
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            default_timeout: Duration::from_secs(30),
        }
    }

    /// 添加预配置的连接
    pub async fn add_connection(&self, connection: DatabaseConnection) -> NodeResult<()> {
        let mut connections = self.connections.write().await;
        connections.insert(connection.name.clone(), connection);
        info!("Database connection added");
        Ok(())
    }

    /// 移除连接
    pub async fn remove_connection(&self, name: &str) -> NodeResult<()> {
        let mut connections = self.connections.write().await;
        if connections.remove(name).is_some() {
            info!("Database connection removed: {}", name);
        }
        Ok(())
    }

    /// 执行数据库命令
    pub async fn execute(&self, cmd: &DatabaseCommand) -> NodeResult<CommandOutput> {
        let db_type = format!("{:?}", cmd.db_type);
        info!("Executing {} query", db_type);

        // 获取连接信息
        let connection = if let Some(name) = &cmd.connection_name {
            let connections = self.connections.read().await;
            connections.get(name).cloned().ok_or_else(|| {
                NodeError::Execution(format!("Database connection '{}' not found", name))
            })?
        } else if let Some(conn_str) = &cmd.connection_string {
            DatabaseConnection {
                name: "ad-hoc".to_string(),
                db_type: cmd.db_type.clone(),
                connection_string: conn_str.clone(),
                max_connections: 1,
                connect_timeout_secs: 30,
                read_only: false,
            }
        } else {
            return Err(NodeError::Execution(
                "Either connection_name or connection_string must be provided".to_string(),
            ));
        };

        // 根据数据库类型执行查询
        match cmd.db_type {
            DatabaseType::Sqlite => self.execute_sqlite(&connection, cmd).await,
            DatabaseType::Postgres => self.execute_postgres(&connection, cmd).await,
            DatabaseType::Mysql => self.execute_mysql(&connection, cmd).await,
            DatabaseType::Mongodb => self.execute_mongodb(&connection, cmd).await,
            DatabaseType::Redis => self.execute_redis(&connection, cmd).await,
        }
    }

    /// 执行 SQLite 查询
    #[cfg(feature = "database-sqlite")]
    async fn execute_sqlite(
        &self,
        connection: &DatabaseConnection,
        cmd: &DatabaseCommand,
    ) -> NodeResult<CommandOutput> {
        use sqlx::sqlite::SqlitePoolOptions;
        use sqlx::{Row, SqlitePool};

        debug!("Executing SQLite query: {}", cmd.query);

        let pool = SqlitePoolOptions::new()
            .max_connections(connection.max_connections)
            .connect_timeout(Duration::from_secs(connection.connect_timeout_secs))
            .connect(&connection.connection_string)
            .await
            .map_err(|e| NodeError::Execution(format!("Failed to connect to SQLite: {}", e)))?;

        // 判断是查询还是执行
        let query_upper = cmd.query.trim().to_uppercase();
        if query_upper.starts_with("SELECT") || query_upper.starts_with("PRAGMA") {
            // 查询操作
            let mut rows = sqlx::query(&cmd.query)
                .fetch_all(&pool)
                .await
                .map_err(|e| NodeError::Execution(format!("Query failed: {}", e)))?;

            let results: Vec<serde_json::Value> = rows
                .iter()
                .map(|row| {
                    let mut map = serde_json::Map::new();
                    for (i, column) in row.columns().iter().enumerate() {
                        let value: Option<String> = row.try_get(i).ok();
                        map.insert(
                            column.name().to_string(),
                            serde_json::Value::String(value.unwrap_or_default()),
                        );
                    }
                    serde_json::Value::Object(map)
                })
                .collect();

            Ok(CommandOutput::json(serde_json::json!({
                "rows": results,
                "row_count": results.len()
            })))
        } else {
            // 执行操作
            let result = sqlx::query(&cmd.query)
                .execute(&pool)
                .await
                .map_err(|e| NodeError::Execution(format!("Execute failed: {}", e)))?;

            Ok(CommandOutput::json(serde_json::json!({
                "rows_affected": result.rows_affected(),
                "last_insert_rowid": result.last_insert_rowid()
            })))
        }
    }

    /// 执行 SQLite 查询 (未启用 feature)
    #[cfg(not(feature = "database-sqlite"))]
    async fn execute_sqlite(
        &self,
        _connection: &DatabaseConnection,
        _cmd: &DatabaseCommand,
    ) -> NodeResult<CommandOutput> {
        Err(NodeError::Execution(
            "SQLite support not enabled. Compile with 'database-sqlite' feature".to_string(),
        ))
    }

    /// 执行 PostgreSQL 查询
    #[cfg(feature = "database-postgres")]
    async fn execute_postgres(
        &self,
        connection: &DatabaseConnection,
        cmd: &DatabaseCommand,
    ) -> NodeResult<CommandOutput> {
        use sqlx::postgres::PgPoolOptions;
        use sqlx::{Row};

        debug!("Executing PostgreSQL query: {}", cmd.query);

        let pool = PgPoolOptions::new()
            .max_connections(connection.max_connections)
            .connect_timeout(Duration::from_secs(connection.connect_timeout_secs))
            .connect(&connection.connection_string)
            .await
            .map_err(|e| NodeError::Execution(format!("Failed to connect to PostgreSQL: {}", e)))?;

        let query_upper = cmd.query.trim().to_uppercase();
        if query_upper.starts_with("SELECT") {
            let rows = sqlx::query(&cmd.query)
                .fetch_all(&pool)
                .await
                .map_err(|e| NodeError::Execution(format!("Query failed: {}", e)))?;

            let results: Vec<serde_json::Value> = rows
                .iter()
                .map(|row| {
                    let mut map = serde_json::Map::new();
                    for (i, column) in row.columns().iter().enumerate() {
                        let value: Option<String> = row.try_get(i).ok();
                        map.insert(
                            column.name().to_string(),
                            serde_json::Value::String(value.unwrap_or_default()),
                        );
                    }
                    serde_json::Value::Object(map)
                })
                .collect();

            Ok(CommandOutput::json(serde_json::json!({
                "rows": results,
                "row_count": results.len()
            })))
        } else {
            let result = sqlx::query(&cmd.query)
                .execute(&pool)
                .await
                .map_err(|e| NodeError::Execution(format!("Execute failed: {}", e)))?;

            Ok(CommandOutput::json(serde_json::json!({
                "rows_affected": result.rows_affected()
            })))
        }
    }

    /// 执行 PostgreSQL 查询 (未启用 feature)
    #[cfg(not(feature = "database-postgres"))]
    async fn execute_postgres(
        &self,
        _connection: &DatabaseConnection,
        _cmd: &DatabaseCommand,
    ) -> NodeResult<CommandOutput> {
        Err(NodeError::Execution(
            "PostgreSQL support not enabled. Compile with 'database-postgres' feature".to_string(),
        ))
    }

    /// 执行 MySQL 查询
    #[cfg(feature = "database-mysql")]
    async fn execute_mysql(
        &self,
        connection: &DatabaseConnection,
        cmd: &DatabaseCommand,
    ) -> NodeResult<CommandOutput> {
        use sqlx::mysql::MySqlPoolOptions;
        use sqlx::{Row};

        debug!("Executing MySQL query: {}", cmd.query);

        let pool = MySqlPoolOptions::new()
            .max_connections(connection.max_connections)
            .connect_timeout(Duration::from_secs(connection.connect_timeout_secs))
            .connect(&connection.connection_string)
            .await
            .map_err(|e| NodeError::Execution(format!("Failed to connect to MySQL: {}", e)))?;

        let query_upper = cmd.query.trim().to_uppercase();
        if query_upper.starts_with("SELECT") {
            let rows = sqlx::query(&cmd.query)
                .fetch_all(&pool)
                .await
                .map_err(|e| NodeError::Execution(format!("Query failed: {}", e)))?;

            let results: Vec<serde_json::Value> = rows
                .iter()
                .map(|row| {
                    let mut map = serde_json::Map::new();
                    for (i, column) in row.columns().iter().enumerate() {
                        let value: Option<String> = row.try_get(i).ok();
                        map.insert(
                            column.name().to_string(),
                            serde_json::Value::String(value.unwrap_or_default()),
                        );
                    }
                    serde_json::Value::Object(map)
                })
                .collect();

            Ok(CommandOutput::json(serde_json::json!({
                "rows": results,
                "row_count": results.len()
            })))
        } else {
            let result = sqlx::query(&cmd.query)
                .execute(&pool)
                .await
                .map_err(|e| NodeError::Execution(format!("Execute failed: {}", e)))?;

            Ok(CommandOutput::json(serde_json::json!({
                "rows_affected": result.rows_affected(),
                "last_insert_id": result.last_insert_id()
            })))
        }
    }

    /// 执行 MySQL 查询 (未启用 feature)
    #[cfg(not(feature = "database-mysql"))]
    async fn execute_mysql(
        &self,
        _connection: &DatabaseConnection,
        _cmd: &DatabaseCommand,
    ) -> NodeResult<CommandOutput> {
        Err(NodeError::Execution(
            "MySQL support not enabled. Compile with 'database-mysql' feature".to_string(),
        ))
    }

    /// 执行 MongoDB 查询
    #[cfg(feature = "database-mongodb")]
    async fn execute_mongodb(
        &self,
        connection: &DatabaseConnection,
        cmd: &DatabaseCommand,
    ) -> NodeResult<CommandOutput> {
        use mongodb::{bson::doc, Client, Collection};
        use mongodb::options::ClientOptions;

        debug!("Executing MongoDB query");

        let client_options = ClientOptions::parse(&connection.connection_string)
            .await
            .map_err(|e| NodeError::Execution(format!("Failed to parse MongoDB URI: {}", e)))?;

        let client = Client::with_options(client_options)
            .map_err(|e| NodeError::Execution(format!("Failed to connect to MongoDB: {}", e)))?;

        // 解析查询 (期望格式: database.collection.find({...}))
        let parts: Vec<&str> = cmd.query.splitn(3, '.').collect();
        if parts.len() < 3 {
            return Err(NodeError::Execution(
                "MongoDB query format: database.collection.operation".to_string(),
            ));
        }

        let db_name = parts[0];
        let collection_name = parts[1];
        let operation = parts[2];

        let db = client.database(db_name);
        let collection: Collection<serde_json::Value> = db.collection(collection_name);

        // 解析操作
        if operation.starts_with("find") {
            let filter = if cmd.params.is_empty() {
                doc! {}
            } else {
                // 简单实现：假设第一个参数是 filter JSON
                doc! {}
            };

            let limit = cmd.limit.unwrap_or(100) as i64;
            let mut cursor = collection
                .find(filter)
                .limit(limit)
                .await
                .map_err(|e| NodeError::Execution(format!("Find failed: {}", e)))?;

            let mut results = Vec::new();
            while cursor.advance().await.map_err(|e| {
                NodeError::Execution(format!("Cursor error: {}", e))
            })? {
                let doc = cursor.deserialize_current().map_err(|e| {
                    NodeError::Execution(format!("Deserialize error: {}", e))
                })?;
                results.push(doc);
            }

            Ok(CommandOutput::json(serde_json::json!({
                "documents": results,
                "count": results.len()
            })))
        } else {
            Err(NodeError::Execution(format!(
                "Unsupported MongoDB operation: {}",
                operation
            )))
        }
    }

    /// 执行 MongoDB 查询 (未启用 feature)
    #[cfg(not(feature = "database-mongodb"))]
    async fn execute_mongodb(
        &self,
        _connection: &DatabaseConnection,
        _cmd: &DatabaseCommand,
    ) -> NodeResult<CommandOutput> {
        Err(NodeError::Execution(
            "MongoDB support not enabled. Compile with 'database-mongodb' feature".to_string(),
        ))
    }

    /// 执行 Redis 命令
    #[cfg(feature = "database-redis")]
    async fn execute_redis(
        &self,
        connection: &DatabaseConnection,
        cmd: &DatabaseCommand,
    ) -> NodeResult<CommandOutput> {
        use redis::AsyncCommands;

        debug!("Executing Redis command: {}", cmd.query);

        let client = redis::Client::open(connection.connection_string.clone())
            .map_err(|e| NodeError::Execution(format!("Failed to create Redis client: {}", e)))?;

        let mut conn = client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| NodeError::Execution(format!("Failed to connect to Redis: {}", e)))?;

        // 解析 Redis 命令
        let parts: Vec<&str> = cmd.query.split_whitespace().collect();
        if parts.is_empty() {
            return Err(NodeError::Execution("Empty Redis command".to_string()));
        }

        let command = parts[0].to_uppercase();
        match command.as_str() {
            "GET" => {
                if parts.len() < 2 {
                    return Err(NodeError::Execution("GET requires a key".to_string()));
                }
                let key = parts[1];
                let value: Option<String> = conn.get(key).await.map_err(|e| {
                    NodeError::Execution(format!("GET failed: {}", e))
                })?;
                Ok(CommandOutput::json(serde_json::json!({
                    "key": key,
                    "value": value
                })))
            }
            "SET" => {
                if parts.len() < 3 {
                    return Err(NodeError::Execution("SET requires key and value".to_string()));
                }
                let key = parts[1];
                let value = parts[2];
                let _: () = conn.set(key, value).await.map_err(|e| {
                    NodeError::Execution(format!("SET failed: {}", e))
                })?;
                Ok(CommandOutput::text("OK"))
            }
            "DEL" => {
                if parts.len() < 2 {
                    return Err(NodeError::Execution("DEL requires keys".to_string()));
                }
                let keys: Vec<&str> = parts[1..].to_vec();
                let count: i64 = conn.del(&keys).await.map_err(|e| {
                    NodeError::Execution(format!("DEL failed: {}", e))
                })?;
                Ok(CommandOutput::json(serde_json::json!({
                    "deleted": count
                })))
            }
            "KEYS" => {
                if parts.len() < 2 {
                    return Err(NodeError::Execution("KEYS requires a pattern".to_string()));
                }
                let pattern = parts[1];
                let keys: Vec<String> = conn.keys(pattern).await.map_err(|e| {
                    NodeError::Execution(format!("KEYS failed: {}", e))
                })?;
                Ok(CommandOutput::json(serde_json::json!({
                    "keys": keys
                })))
            }
            _ => Err(NodeError::Execution(format!(
                "Unsupported Redis command: {}",
                command
            ))),
        }
    }

    /// 执行 Redis 命令 (未启用 feature)
    #[cfg(not(feature = "database-redis"))]
    async fn execute_redis(
        &self,
        _connection: &DatabaseConnection,
        _cmd: &DatabaseCommand,
    ) -> NodeResult<CommandOutput> {
        Err(NodeError::Execution(
            "Redis support not enabled. Compile with 'database-redis' feature".to_string(),
        ))
    }
}

impl Default for DatabaseExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for DatabaseExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DatabaseExecutor")
            .field("default_timeout", &self.default_timeout)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_add_connection() {
        let executor = DatabaseExecutor::new();
        let conn = DatabaseConnection {
            name: "test".to_string(),
            db_type: DatabaseType::Sqlite,
            connection_string: ":memory:".to_string(),
            max_connections: 1,
            connect_timeout_secs: 5,
            read_only: false,
        };

        executor.add_connection(conn).await.unwrap();
    }
}

//! Data export for GDPR compliance
//!
//! 用户数据导出功能 (GDPR Article 20: 数据携带权)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};
use uuid::Uuid;

use super::Result;

/// 数据导出格式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataExportFormat {
    /// JSON 格式
    Json,
    /// CSV 格式
    Csv,
    /// XML 格式
    Xml,
    /// PDF 格式 (人类可读)
    Pdf,
}

impl Default for DataExportFormat {
    fn default() -> Self {
        Self::Json
    }
}

/// 数据导出请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataExportRequest {
    /// 请求 ID
    pub id: Uuid,
    /// 用户 ID
    pub user_id: String,
    /// 租户 ID
    pub tenant_id: String,
    /// 导出格式
    pub format: DataExportFormat,
    /// 请求的数据类别
    pub data_categories: Vec<String>,
    /// 创建时间
    pub created_at: u64,
    /// 过期时间
    pub expires_at: Option<u64>,
    /// 状态
    pub status: ExportStatus,
}

/// 导出状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExportStatus {
    /// 待处理
    Pending,
    /// 处理中
    Processing,
    /// 已完成
    Completed,
    /// 失败
    Failed,
    /// 已过期
    Expired,
}

impl DataExportRequest {
    /// 创建新的导出请求
    pub fn new(
        user_id: impl Into<String>,
        tenant_id: impl Into<String>,
        format: DataExportFormat,
        data_categories: Vec<String>,
    ) -> Self {
        let now = chrono::Utc::now().timestamp_millis() as u64;
        let expires_at = Some(now + 7 * 24 * 60 * 60 * 1000); // 7 天后过期

        Self {
            id: Uuid::new_v4(),
            user_id: user_id.into(),
            tenant_id: tenant_id.into(),
            format,
            data_categories,
            created_at: now,
            expires_at,
            status: ExportStatus::Pending,
        }
    }
}

/// 导出结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportResult {
    /// 请求 ID
    pub request_id: Uuid,
    /// 用户 ID
    pub user_id: String,
    /// 导出数据
    pub data: HashMap<String, serde_json::Value>,
    /// 元数据
    pub metadata: ExportMetadata,
    /// 导出时间
    pub exported_at: u64,
    /// 数据校验和
    pub checksum: String,
}

/// 导出元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportMetadata {
    /// 导出格式
    pub format: String,
    /// 数据大小 (字节)
    pub size_bytes: u64,
    /// 记录数量
    pub record_count: u64,
    /// 包含的数据类别
    pub categories: Vec<String>,
    /// 数据时间范围
    pub date_range: Option<DateRange>,
    /// 生成时间
    pub generated_at: u64,
}

/// 日期范围
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateRange {
    pub start: u64,
    pub end: u64,
}

/// 数据提供者 trait
#[async_trait::async_trait]
pub trait DataProvider: Send + Sync {
    /// 获取用户数据
    async fn get_user_data(
        &self,
        user_id: &str,
        tenant_id: &str,
        category: &str,
    ) -> std::result::Result<Vec<serde_json::Value>, anyhow::Error>;
}

/// 数据导出管理器
pub struct DataExportManager {
    /// 数据提供者 (category -> provider)
    providers: Arc<RwLock<HashMap<String, Arc<dyn DataProvider>>>>,
    /// 导出请求缓存
    requests: Arc<RwLock<HashMap<Uuid, DataExportRequest>>>,
    /// 导出结果缓存
    results: Arc<RwLock<HashMap<Uuid, ExportResult>>>,
}

impl DataExportManager {
    /// 创建新的导出管理器
    pub fn new() -> Self {
        Self {
            providers: Arc::new(RwLock::new(HashMap::new())),
            requests: Arc::new(RwLock::new(HashMap::new())),
            results: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 注册数据提供者
    pub async fn register_provider(&self, category: impl Into<String>, provider: Arc<dyn DataProvider>) {
        let mut providers = self.providers.write().await;
        providers.insert(category.into(), provider);
    }

    /// 创建导出请求
    pub async fn create_request(
        &self,
        user_id: &str,
        tenant_id: &str,
        format: DataExportFormat,
        data_categories: Vec<String>,
    ) -> Result<DataExportRequest> {
        let request = DataExportRequest::new(user_id, tenant_id, format, data_categories);

        let mut requests = self.requests.write().await;
        requests.insert(request.id, request.clone());

        info!("Created data export request {} for user {}", request.id, user_id);
        Ok(request)
    }

    /// 执行数据导出
    pub async fn execute_export(&self, request_id: &Uuid) -> Result<ExportResult> {
        // 获取请求
        let request = {
            let mut requests = self.requests.write().await;
            let req = requests
                .get_mut(request_id)
                .ok_or_else(|| super::GdprError::ExportFailed("Request not found".to_string()))?;

            if req.status != ExportStatus::Pending {
                return Err(super::GdprError::ExportFailed(format!(
                    "Invalid request status: {:?}",
                    req.status
                )));
            }

            req.status = ExportStatus::Processing;
            req.clone()
        };

        // 收集数据
        let mut exported_data = HashMap::new();
        let mut total_records = 0u64;
        let mut total_size = 0u64;

        let providers = self.providers.read().await;

        for category in &request.data_categories {
            if let Some(provider) = providers.get(category) {
                match provider
                    .get_user_data(&request.user_id, &request.tenant_id, category)
                    .await
                {
                    Ok(data) => {
                        total_records += data.len() as u64;
                        let value = serde_json::to_value(&data).unwrap_or(serde_json::json!([]));
                        total_size += serde_json::to_string(&value)
                            .map(|s| s.len() as u64)
                            .unwrap_or(0);
                        exported_data.insert(category.clone(), value);
                        debug!("Exported {} records for category {}", data.len(), category);
                    }
                    Err(e) => {
                        debug!("Failed to export category {}: {}", category, e);
                        exported_data.insert(
                            category.clone(),
                            serde_json::json!({
                                "error": e.to_string(),
                                "category": category
                            }),
                        );
                    }
                }
            } else {
                debug!("No provider registered for category: {}", category);
            }
        }

        // 生成校验和 (使用简单哈希)
        let data_string = serde_json::to_string(&exported_data)
            .map_err(|e| super::GdprError::ExportFailed(e.to_string()))?;
        let checksum = format!("{:x}", data_string.len() as u32 ^ data_string.bytes().fold(0u32, |acc, b| acc.wrapping_add(b as u32)));

        // 创建结果
        let result = ExportResult {
            request_id: request.id,
            user_id: request.user_id.clone(),
            data: exported_data,
            metadata: ExportMetadata {
                format: "json".to_string(),
                size_bytes: total_size,
                record_count: total_records,
                categories: request.data_categories.clone(),
                date_range: None,
                generated_at: chrono::Utc::now().timestamp_millis() as u64,
            },
            exported_at: chrono::Utc::now().timestamp_millis() as u64,
            checksum,
        };

        // 更新状态
        {
            let mut requests = self.requests.write().await;
            if let Some(req) = requests.get_mut(request_id) {
                req.status = ExportStatus::Completed;
            }
        }

        // 缓存结果
        {
            let mut results = self.results.write().await;
            results.insert(*request_id, result.clone());
        }

        info!(
            "Data export completed for request {} with {} records",
            request_id, total_records
        );
        Ok(result)
    }

    /// 获取导出结果
    pub async fn get_result(&self, request_id: &Uuid) -> Option<ExportResult> {
        let results = self.results.read().await;
        results.get(request_id).cloned()
    }

    /// 获取请求状态
    pub async fn get_request_status(&self, request_id: &Uuid) -> Option<ExportStatus> {
        let requests = self.requests.read().await;
        requests.get(request_id).map(|r| r.status)
    }

    /// 取消导出请求
    pub async fn cancel_request(&self, request_id: &Uuid) -> Result<bool> {
        let mut requests = self.requests.write().await;
        if let Some(request) = requests.remove(request_id) {
            // 同时移除结果
            let mut results = self.results.write().await;
            results.remove(request_id);

            info!("Cancelled export request {} for user {}", request_id, request.user_id);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// 将导出结果序列化为指定格式
    pub fn serialize_result(result: &ExportResult, format: DataExportFormat) -> Result<Vec<u8>> {
        match format {
            DataExportFormat::Json => {
                serde_json::to_vec_pretty(&result)
                    .map_err(|e| super::GdprError::ExportFailed(e.to_string()))
            }
            DataExportFormat::Csv => {
                // 简化实现：将 JSON 转换为 CSV 格式
                let mut csv_output = String::new();
                csv_output.push_str("category,data,exported_at\n");

                for (category, data) in &result.data {
                    let data_str = serde_json::to_string(data).unwrap_or_default();
                    csv_output.push_str(&format!("{},{},{}\n", category, data_str, result.exported_at));
                }

                Ok(csv_output.into_bytes())
            }
            DataExportFormat::Xml => {
                // 简化实现：将 JSON 转换为 XML 格式
                let mut xml_output = String::new();
                xml_output.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
                xml_output.push_str("<data_export>\n");

                for (category, data) in &result.data {
                    xml_output.push_str(&format!("  <category name=\"{}\">\n", category));
                    xml_output.push_str(&format!(
                        "    {}\n",
                        serde_json::to_string(data).unwrap_or_default()
                    ));
                    xml_output.push_str("  </category>\n");
                }

                xml_output.push_str("</data_export>");
                Ok(xml_output.into_bytes())
            }
            DataExportFormat::Pdf => {
                // PDF 生成需要额外的库支持
                Err(super::GdprError::ExportFailed(
                    "PDF format not yet supported".to_string(),
                ))
            }
        }
    }
}

impl Default for DataExportManager {
    fn default() -> Self {
        Self::new()
    }
}

// 添加 md5 依赖模拟 (实际应使用 md5 crate)
mod md5 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    pub struct Md5;

    impl Md5 {
        pub fn compute(data: &[u8]) -> u64 {
            let mut hasher = DefaultHasher::new();
            data.hash(&mut hasher);
            hasher.finish()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_export_request_creation() {
        let request = DataExportRequest::new(
            "user-1",
            "tenant-1",
            DataExportFormat::Json,
            vec!["profile".to_string(), "messages".to_string()],
        );

        assert_eq!(request.status, ExportStatus::Pending);
        assert!(request.expires_at.is_some());
    }

    #[tokio::test]
    async fn test_export_manager() {
        let manager = DataExportManager::new();

        let request = manager
            .create_request(
                "user-1",
                "tenant-1",
                DataExportFormat::Json,
                vec!["profile".to_string()],
            )
            .await
            .unwrap();

        assert!(manager.get_result(&request.id).await.is_none());
    }
}

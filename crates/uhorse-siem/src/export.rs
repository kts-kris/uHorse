//! Audit Log Export
//!
//! 支持多种格式导出审计日志 (CEF/JSON)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Write;

/// 导出格式
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExportFormat {
    /// JSON 格式
    Json,
    /// CEF (Common Event Format)
    Cef,
    /// Syslog 格式
    Syslog,
    /// CSV 格式
    Csv,
}

/// 审计事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    /// 事件 ID
    pub id: String,
    /// 时间戳
    pub timestamp: DateTime<Utc>,
    /// 事件类型
    pub event_type: String,
    /// 租户 ID
    pub tenant_id: String,
    /// 执行者
    pub actor: String,
    /// 资源
    pub resource: String,
    /// 操作
    pub action: String,
    /// 结果
    pub result: String,
    /// 详情
    #[serde(default)]
    pub details: HashMap<String, serde_json::Value>,
    /// IP 地址
    pub ip_address: Option<String>,
    /// User Agent
    pub user_agent: Option<String>,
    /// 签名 (防篡改)
    pub signature: Option<String>,
}

impl AuditEvent {
    /// 创建新的审计事件
    pub fn new(
        event_type: impl Into<String>,
        tenant_id: impl Into<String>,
        actor: impl Into<String>,
        resource: impl Into<String>,
        action: impl Into<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            event_type: event_type.into(),
            tenant_id: tenant_id.into(),
            actor: actor.into(),
            resource: resource.into(),
            action: action.into(),
            result: "success".to_string(),
            details: HashMap::new(),
            ip_address: None,
            user_agent: None,
            signature: None,
        }
    }

    /// 设置结果
    pub fn with_result(mut self, result: impl Into<String>) -> Self {
        self.result = result.into();
        self
    }

    /// 设置 IP 地址
    pub fn with_ip(mut self, ip: impl Into<String>) -> Self {
        self.ip_address = Some(ip.into());
        self
    }

    /// 设置 User Agent
    pub fn with_user_agent(mut self, ua: impl Into<String>) -> Self {
        self.user_agent = Some(ua.into());
        self
    }

    /// 添加详情
    pub fn add_detail(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.details.insert(key.into(), value);
        self
    }

    /// 计算 CEF 严重性
    pub fn cef_severity(&self) -> u8 {
        match self.result.as_str() {
            "failure" | "denied" => 8,
            "error" => 7,
            "warning" => 5,
            _ => 3,
        }
    }
}

/// 审计导出器
pub struct AuditExporter {
    /// 导出格式
    format: ExportFormat,
}

impl AuditExporter {
    /// 创建新的导出器
    pub fn new(format: ExportFormat) -> Self {
        Self { format }
    }

    /// 导出单个事件
    pub fn export_event(&self, event: &AuditEvent) -> Result<String, crate::SiemError> {
        match self.format {
            ExportFormat::Json => self.export_json(event),
            ExportFormat::Cef => self.export_cef(event),
            ExportFormat::Syslog => self.export_syslog(event),
            ExportFormat::Csv => self.export_csv(event),
        }
    }

    /// 导出多个事件
    pub fn export_events(&self, events: &[AuditEvent]) -> Result<String, crate::SiemError> {
        match self.format {
            ExportFormat::Json => {
                let json = serde_json::to_string_pretty(events)?;
                Ok(json)
            }
            _ => {
                let mut output = String::new();
                for event in events {
                    output.push_str(&self.export_event(event)?);
                    output.push('\n');
                }
                Ok(output)
            }
        }
    }

    /// 导出为 JSON
    fn export_json(&self, event: &AuditEvent) -> Result<String, crate::SiemError> {
        Ok(serde_json::to_string(event)?)
    }

    /// 导出为 CEF 格式
    fn export_cef(&self, event: &AuditEvent) -> Result<String, crate::SiemError> {
        // CEF: Version|Device Vendor|Device Product|Device Version|Signature ID|Name|Severity|Extension
        let extension = self.build_cef_extension(event);

        Ok(format!(
            "CEF:0|uHorse|AI Gateway|2.0|{}|{}|{}|{}",
            event.event_type,
            format!("{} {}", event.action, event.resource),
            event.cef_severity(),
            extension
        ))
    }

    /// 构建 CEF 扩展
    fn build_cef_extension(&self, event: &AuditEvent) -> String {
        let mut parts = vec![
            format!("rt={}", event.timestamp.timestamp_millis()),
            format!("suser={}", event.actor),
            format!("duser={}", event.tenant_id),
            format!("request={}", event.resource),
            format!("act={}", event.action),
            format!("outcome={}", event.result),
        ];

        if let Some(ref ip) = event.ip_address {
            parts.push(format!("src={}", ip));
        }

        if let Some(ref ua) = event.user_agent {
            parts.push(format!("requestClientApplication={}", ua));
        }

        parts.join(" ")
    }

    /// 导出为 Syslog 格式
    fn export_syslog(&self, event: &AuditEvent) -> Result<String, crate::SiemError> {
        let priority = 13 * 8 + 6; // Facility: local0, Severity: info

        Ok(format!(
            "<{}>{} {} uhorse[{}]: [{}] {} {} {} {} {}",
            priority,
            event.timestamp.format("%b %d %H:%M:%S"),
            "localhost",
            std::process::id(),
            event.event_type,
            event.tenant_id,
            event.actor,
            event.resource,
            event.action,
            event.result
        ))
    }

    /// 导出为 CSV 格式
    fn export_csv(&self, event: &AuditEvent) -> Result<String, crate::SiemError> {
        let details = serde_json::to_string(&event.details).unwrap_or_default();

        Ok(format!(
            "{},{},{},{},{},{},{},{},{},{},{}",
            event.id,
            event.timestamp.to_rfc3339(),
            event.event_type,
            event.tenant_id,
            event.actor,
            event.resource,
            event.action,
            event.result,
            event.ip_address.as_deref().unwrap_or(""),
            event.user_agent.as_deref().unwrap_or(""),
            details
        ))
    }

    /// 获取 CSV 头
    pub fn csv_header() -> String {
        "id,timestamp,event_type,tenant_id,actor,resource,action,result,ip_address,user_agent,details".to_string()
    }

    /// 写入文件
    pub fn write_to_file(
        &self,
        events: &[AuditEvent],
        path: &std::path::Path,
    ) -> crate::Result<()> {
        let content = self.export_events(events)?;
        let mut file = std::fs::File::create(path)?;
        file.write_all(content.as_bytes())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_event_creation() {
        let event = AuditEvent::new(
            "user.login",
            "tenant-001",
            "user-123",
            "/auth/login",
            "login",
        )
        .with_ip("192.168.1.1");

        assert!(!event.id.is_empty());
        assert_eq!(event.event_type, "user.login");
        assert_eq!(event.ip_address, Some("192.168.1.1".to_string()));
    }

    #[test]
    fn test_export_json() {
        let event = AuditEvent::new(
            "user.login",
            "tenant-001",
            "user-123",
            "/auth/login",
            "login",
        );

        let exporter = AuditExporter::new(ExportFormat::Json);
        let json = exporter.export_event(&event).unwrap();

        assert!(json.contains("user.login"));
        assert!(json.contains("tenant-001"));
    }

    #[test]
    fn test_export_cef() {
        let event = AuditEvent::new(
            "user.login",
            "tenant-001",
            "user-123",
            "/auth/login",
            "login",
        )
        .with_result("success");

        let exporter = AuditExporter::new(ExportFormat::Cef);
        let cef = exporter.export_event(&event).unwrap();

        assert!(cef.starts_with("CEF:0|uHorse"));
        assert!(cef.contains("user.login"));
    }

    #[test]
    fn test_export_csv() {
        let event = AuditEvent::new(
            "user.login",
            "tenant-001",
            "user-123",
            "/auth/login",
            "login",
        );

        let exporter = AuditExporter::new(ExportFormat::Csv);
        let csv = exporter.export_event(&event).unwrap();

        assert!(csv.contains("user.login"));
        assert!(csv.contains("tenant-001"));
    }
}

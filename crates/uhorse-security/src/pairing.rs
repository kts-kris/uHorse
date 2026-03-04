//! # 设备配对管理
//!
//! 完整的设备配对流程，包括配对协议、状态管理和 UI 流程。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uhorse_core::{DeviceId, DeviceInfo, DeviceManager, Result, UHorseError};
use uuid::Uuid;

/// 配对状态
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PairingStatus {
    /// 待配对
    Pending,
    /// 等待确认
    AwaitingConfirmation,
    /// 已配对
    Paired,
    /// 已拒绝
    Rejected,
    /// 已过期
    Expired,
    /// 已取消
    Cancelled,
}

/// 配对请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingRequest {
    /// 请求 ID
    pub request_id: String,
    /// 设备 ID
    pub device_id: DeviceId,
    /// 设备名称
    pub device_name: String,
    /// 设备类型
    pub device_type: String,
    /// 用户 ID
    pub user_id: Option<String>,
    /// 配对码
    pub pairing_code: String,
    /// 状态
    pub status: PairingStatus,
    /// 创建时间
    pub created_at: u64,
    /// 过期时间
    pub expires_at: u64,
    /// 配对元数据
    pub metadata: serde_json::Value,
}

impl PairingRequest {
    /// 创建新的配对请求
    pub fn new(device_id: DeviceId, device_name: String, device_type: String) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let expires_at = now + 300; // 5 分钟过期

        Self {
            request_id: Uuid::new_v4().to_string(),
            device_id,
            device_name,
            device_type,
            user_id: None,
            pairing_code: Self::generate_pairing_code(),
            status: PairingStatus::Pending,
            created_at: now,
            expires_at,
            metadata: serde_json::json!({}),
        }
    }

    /// 生成 6 位配对码
    fn generate_pairing_code() -> String {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        format!("{:06}", rng.gen_range(0..1000000))
    }

    /// 检查是否过期
    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        now > self.expires_at
    }

    /// 确认配对
    pub fn confirm(&mut self, user_id: String) {
        self.status = PairingStatus::Paired;
        self.user_id = Some(user_id);
    }

    /// 拒绝配对
    pub fn reject(&mut self) {
        self.status = PairingStatus::Rejected;
    }

    /// 取消配对
    pub fn cancel(&mut self) {
        self.status = PairingStatus::Cancelled;
    }
}

/// 设备配对管理器
#[derive(Debug)]
pub struct DevicePairingManager {
    /// 设备信息存储
    devices: Arc<RwLock<HashMap<DeviceId, DeviceInfo>>>,
    /// 活跃的配对请求
    pairing_requests: Arc<RwLock<HashMap<String, PairingRequest>>>,
    /// 配对码到请求 ID 的映射
    code_to_request: Arc<RwLock<HashMap<String, String>>>,
    /// 配对请求过期时间（秒）
    pairing_ttl: u64,
}

impl DevicePairingManager {
    pub fn new() -> Self {
        Self {
            devices: Arc::new(RwLock::new(HashMap::new())),
            pairing_requests: Arc::new(RwLock::new(HashMap::new())),
            code_to_request: Arc::new(RwLock::new(HashMap::new())),
            pairing_ttl: 300, // 默认 5 分钟
        }
    }

    /// 设置配对请求过期时间
    pub fn with_pairing_ttl(mut self, ttl: u64) -> Self {
        self.pairing_ttl = ttl;
        self
    }

    /// 发起配对请求
    pub async fn initiate_pairing(
        &self,
        device_id: DeviceId,
        device_name: String,
        device_type: String,
    ) -> Result<PairingRequest> {
        let mut request = PairingRequest::new(device_id.clone(), device_name, device_type);

        // 先注册设备
        let device_info = DeviceInfo {
            id: device_id.clone(),
            name: request.device_name.clone(),
            paired: false,
            paired_at: None,
            last_seen: request.created_at,
            capabilities: uhorse_core::DeviceCapabilities::default(),
        };

        self.devices
            .write()
            .await
            .insert(device_id.clone(), device_info);

        // 存储配对请求
        let pairing_code = request.pairing_code.clone();
        let request_id = request.request_id.clone();
        let request_id_copy = request_id.clone();
        let request_id_copy2 = request_id.clone();

        self.pairing_requests
            .write()
            .await
            .insert(request_id.clone(), request.clone());
        self.code_to_request
            .write()
            .await
            .insert(pairing_code, request_id);

        info!(
            "Initiated pairing request: {} for device: {}",
            request_id_copy, device_id
        );

        // 启动过期清理任务
        let requests = Arc::clone(&self.pairing_requests);
        let codes = Arc::clone(&self.code_to_request);
        let req_id = request_id_copy2;
        let expires_at = request.expires_at;

        tokio::spawn(async move {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();

            if expires_at > now {
                tokio::time::sleep(tokio::time::Duration::from_secs(expires_at - now)).await;
            }

            // 过期后清理
            if let Some(mut req) = requests.write().await.get_mut(&req_id) {
                if req.status == PairingStatus::Pending
                    || req.status == PairingStatus::AwaitingConfirmation
                {
                    req.status = PairingStatus::Expired;
                    // 清理代码映射
                    codes.write().await.remove(&req.pairing_code);
                }
            }
        });

        Ok(request)
    }

    /// 通过配对码获取配对请求
    pub async fn get_request_by_code(&self, code: &str) -> Result<PairingRequest> {
        let codes = self.code_to_request.read().await;
        let request_id = codes
            .get(code)
            .ok_or_else(|| UHorseError::InternalError("Invalid pairing code".to_string()))?;

        let requests = self.pairing_requests.read().await;
        let request = requests
            .get(request_id)
            .ok_or_else(|| UHorseError::InternalError("Pairing request not found".to_string()))?;

        Ok(request.clone())
    }

    /// 确认配对
    pub async fn confirm_pairing(&self, code: &str, user_id: String) -> Result<DeviceInfo> {
        let codes = self.code_to_request.read().await;
        let request_id = codes
            .get(code)
            .ok_or_else(|| UHorseError::InternalError("Invalid pairing code".to_string()))?
            .clone();

        let mut requests = self.pairing_requests.write().await;
        let request = requests
            .get_mut(&request_id)
            .ok_or_else(|| UHorseError::InternalError("Pairing request not found".to_string()))?;

        if request.is_expired() {
            request.status = PairingStatus::Expired;
            return Err(UHorseError::InternalError(
                "Pairing request expired".to_string(),
            ));
        }

        request.confirm(user_id.clone());

        // 更新设备状态
        let mut devices = self.devices.write().await;
        if let Some(device) = devices.get_mut(&request.device_id) {
            device.paired = true;
            device.paired_at = Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            );

            info!(
                "Device paired: {} for user: {}",
                request.device_id.0, user_id
            );

            // 清理配对请求
            codes.clone(); // 释放锁
            drop(codes);
            self.code_to_request.write().await.remove(code);

            Ok(device.clone())
        } else {
            Err(UHorseError::DeviceNotPaired(request.device_id.clone()))
        }
    }

    /// 拒绝配对
    pub async fn reject_pairing(&self, code: &str) -> Result<()> {
        let codes = self.code_to_request.read().await;
        let request_id = codes
            .get(code)
            .ok_or_else(|| UHorseError::InternalError("Invalid pairing code".to_string()))?
            .clone();

        let mut requests = self.pairing_requests.write().await;
        if let Some(request) = requests.get_mut(&request_id) {
            request.reject();
            info!("Pairing rejected: {}", request_id);

            // 清理配对请求
            drop(codes);
            self.code_to_request.write().await.remove(code);

            Ok(())
        } else {
            Err(UHorseError::InternalError(
                "Pairing request not found".to_string(),
            ))
        }
    }

    /// 取消配对
    pub async fn cancel_pairing(&self, request_id: &str) -> Result<()> {
        let mut requests = self.pairing_requests.write().await;
        if let Some(request) = requests.get_mut(request_id) {
            request.cancel();

            // 清理代码映射
            self.code_to_request
                .write()
                .await
                .remove(&request.pairing_code);

            info!("Pairing cancelled: {}", request_id);
            Ok(())
        } else {
            Err(UHorseError::InternalError(
                "Pairing request not found".to_string(),
            ))
        }
    }

    /// 获取配对请求
    pub async fn get_pairing_request(&self, request_id: &str) -> Result<PairingRequest> {
        let requests = self.pairing_requests.read().await;
        requests
            .get(request_id)
            .cloned()
            .ok_or_else(|| UHorseError::InternalError("Pairing request not found".to_string()))
    }

    /// 列出待处理的配对请求
    pub async fn list_pending_requests(&self) -> Result<Vec<PairingRequest>> {
        let requests = self.pairing_requests.read().await;
        Ok(requests
            .values()
            .filter(|r| {
                r.status == PairingStatus::Pending
                    || r.status == PairingStatus::AwaitingConfirmation
            })
            .cloned()
            .collect())
    }

    /// 清理过期的配对请求
    pub async fn cleanup_expired_requests(&self) -> Result<usize> {
        let mut requests = self.pairing_requests.write().await;
        let mut codes = self.code_to_request.write().await;

        let expired: Vec<String> = requests
            .iter()
            .filter(|(_, r)| {
                r.is_expired()
                    || r.status == PairingStatus::Rejected
                    || r.status == PairingStatus::Cancelled
            })
            .map(|(id, _)| id.clone())
            .collect();

        for id in &expired {
            if let Some(request) = requests.remove(id) {
                codes.remove(&request.pairing_code);
            }
        }

        let count = expired.len();
        if count > 0 {
            debug!("Cleaned up {} expired pairing requests", count);
        }

        Ok(count)
    }

    /// 获取配对状态
    pub async fn get_pairing_status(&self, device_id: &DeviceId) -> Result<PairingStatus> {
        let devices = self.devices.read().await;
        if let Some(device) = devices.get(device_id) {
            if device.paired {
                Ok(PairingStatus::Paired)
            } else {
                // 检查是否有待处理的配对请求
                let requests = self.pairing_requests.read().await;
                for request in requests.values() {
                    if &request.device_id == device_id {
                        return Ok(request.status.clone());
                    }
                }
                Ok(PairingStatus::Pending)
            }
        } else {
            Err(UHorseError::DeviceNotPaired(device_id.clone()))
        }
    }
}

impl Default for DevicePairingManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pairing_request_creation() {
        let request = PairingRequest::new(
            DeviceId("device-001".to_string()),
            "Test Device".to_string(),
            "sensor".to_string(),
        );

        assert!(!request.request_id.is_empty());
        assert_eq!(request.device_id.0, "device-001");
        assert_eq!(request.device_name, "Test Device");
        assert_eq!(request.device_type, "sensor");
        assert_eq!(request.status, PairingStatus::Pending);
        assert_eq!(request.pairing_code.len(), 6);
        assert!(!request.is_expired());
    }

    #[test]
    fn test_pairing_request_expiration() {
        let mut request = PairingRequest::new(
            DeviceId("device-001".to_string()),
            "Test".to_string(),
            "type".to_string(),
        );

        // 设置过期时间为过去
        request.expires_at = 0;
        assert!(request.is_expired());
    }

    #[test]
    fn test_pairing_request_confirm() {
        let mut request = PairingRequest::new(
            DeviceId("device-001".to_string()),
            "Test".to_string(),
            "type".to_string(),
        );

        request.confirm("user-123".to_string());

        assert_eq!(request.status, PairingStatus::Paired);
        assert_eq!(request.user_id, Some("user-123".to_string()));
    }

    #[test]
    fn test_pairing_request_reject() {
        let mut request = PairingRequest::new(
            DeviceId("device-001".to_string()),
            "Test".to_string(),
            "type".to_string(),
        );

        request.reject();
        assert_eq!(request.status, PairingStatus::Rejected);
    }

    #[test]
    fn test_pairing_request_cancel() {
        let mut request = PairingRequest::new(
            DeviceId("device-001".to_string()),
            "Test".to_string(),
            "type".to_string(),
        );

        request.cancel();
        assert_eq!(request.status, PairingStatus::Cancelled);
    }

    #[tokio::test]
    async fn test_initiate_pairing() {
        let manager = DevicePairingManager::new();

        let request = manager
            .initiate_pairing(
                DeviceId("device-001".to_string()),
                "My Device".to_string(),
                "sensor".to_string(),
            )
            .await
            .unwrap();

        assert!(!request.request_id.is_empty());
        assert_eq!(request.device_id.0, "device-001");
        assert_eq!(request.status, PairingStatus::Pending);
    }

    #[tokio::test]
    async fn test_get_request_by_code() {
        let manager = DevicePairingManager::new();

        let request = manager
            .initiate_pairing(
                DeviceId("device-001".to_string()),
                "My Device".to_string(),
                "sensor".to_string(),
            )
            .await
            .unwrap();

        let code = request.pairing_code.clone();
        let retrieved = manager.get_request_by_code(&code).await.unwrap();

        assert_eq!(retrieved.request_id, request.request_id);
    }

    #[tokio::test]
    async fn test_get_request_by_invalid_code_fails() {
        let manager = DevicePairingManager::new();

        let result = manager.get_request_by_code("invalid").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_confirm_pairing() {
        let manager = DevicePairingManager::new();

        let request = manager
            .initiate_pairing(
                DeviceId("device-001".to_string()),
                "My Device".to_string(),
                "sensor".to_string(),
            )
            .await
            .unwrap();

        let code = request.pairing_code.clone();
        let device = manager
            .confirm_pairing(&code, "user-123".to_string())
            .await
            .unwrap();

        assert!(device.paired);
        assert!(device.paired_at.is_some());

        // 验证配对码已被清除
        let result = manager.get_request_by_code(&code).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_reject_pairing() {
        let manager = DevicePairingManager::new();

        let request = manager
            .initiate_pairing(
                DeviceId("device-001".to_string()),
                "My Device".to_string(),
                "sensor".to_string(),
            )
            .await
            .unwrap();

        let code = request.pairing_code.clone();
        manager.reject_pairing(&code).await.unwrap();

        // 验证配对码已被清除
        let result = manager.get_request_by_code(&code).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_cancel_pairing() {
        let manager = DevicePairingManager::new();

        let request = manager
            .initiate_pairing(
                DeviceId("device-001".to_string()),
                "My Device".to_string(),
                "sensor".to_string(),
            )
            .await
            .unwrap();

        let request_id = request.request_id.clone();
        manager.cancel_pairing(&request_id).await.unwrap();

        let retrieved = manager.get_pairing_request(&request_id).await.unwrap();
        assert_eq!(retrieved.status, PairingStatus::Cancelled);
    }

    #[tokio::test]
    async fn test_list_pending_requests() {
        let manager = DevicePairingManager::new();

        // 创建多个配对请求
        manager
            .initiate_pairing(
                DeviceId("device-001".to_string()),
                "Device 1".to_string(),
                "sensor".to_string(),
            )
            .await
            .unwrap();
        manager
            .initiate_pairing(
                DeviceId("device-002".to_string()),
                "Device 2".to_string(),
                "actuator".to_string(),
            )
            .await
            .unwrap();

        let pending = manager.list_pending_requests().await.unwrap();
        assert_eq!(pending.len(), 2);
    }

    #[tokio::test]
    async fn test_get_pairing_status() {
        let manager = DevicePairingManager::new();

        let device_id = DeviceId("device-001".to_string());
        manager
            .initiate_pairing(
                device_id.clone(),
                "Device 1".to_string(),
                "sensor".to_string(),
            )
            .await
            .unwrap();

        let status = manager.get_pairing_status(&device_id).await.unwrap();
        assert_eq!(status, PairingStatus::Pending);
    }

    #[tokio::test]
    async fn test_device_manager_trait_register() {
        let manager = DevicePairingManager::new();
        let device = DeviceInfo {
            id: DeviceId("device-001".to_string()),
            name: "Test Device".to_string(),
            paired: false,
            paired_at: None,
            last_seen: 0,
            capabilities: uhorse_core::DeviceCapabilities::default(),
        };

        manager.register_device(&device).await.unwrap();
        let retrieved = manager.get_device(&device.id).await.unwrap();
        assert!(retrieved.is_some());
    }

    #[tokio::test]
    async fn test_device_manager_trait_pair_device() {
        let manager = DevicePairingManager::new();
        let device_id = DeviceId("device-001".to_string());
        let device = DeviceInfo {
            id: device_id.clone(),
            name: "Test Device".to_string(),
            paired: false,
            paired_at: None,
            last_seen: 0,
            capabilities: uhorse_core::DeviceCapabilities::default(),
        };

        manager.register_device(&device).await.unwrap();
        manager.pair_device(&device_id).await.unwrap();

        let paired = manager.get_device(&device_id).await.unwrap().unwrap();
        assert!(paired.paired);
    }
}

#[async_trait::async_trait]
impl DeviceManager for DevicePairingManager {
    async fn register_device(&self, device: &DeviceInfo) -> Result<()> {
        self.devices
            .write()
            .await
            .insert(device.id.clone(), device.clone());
        Ok(())
    }

    async fn get_device(&self, id: &DeviceId) -> Result<Option<DeviceInfo>> {
        Ok(self.devices.read().await.get(id).cloned())
    }

    async fn update_device(&self, device: &DeviceInfo) -> Result<()> {
        self.devices
            .write()
            .await
            .insert(device.id.clone(), device.clone());
        Ok(())
    }

    async fn delete_device(&self, id: &DeviceId) -> Result<()> {
        self.devices.write().await.remove(id);
        Ok(())
    }

    async fn pair_device(&self, id: &DeviceId) -> Result<()> {
        let mut devices = self.devices.write().await;
        if let Some(device) = devices.get_mut(id) {
            let mut updated = device.clone();
            updated.paired = true;
            updated.paired_at = Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            );
            *device = updated;
            info!("Device paired via direct pair: {}", id.0);
            Ok(())
        } else {
            Err(UHorseError::DeviceNotPaired(id.clone()))
        }
    }

    async fn unpair_device(&self, id: &DeviceId) -> Result<()> {
        let mut devices = self.devices.write().await;
        if let Some(device) = devices.get_mut(id) {
            let mut updated = device.clone();
            updated.paired = false;
            updated.paired_at = None;
            *device = updated;
            info!("Device unpaired: {}", id.0);
            Ok(())
        } else {
            Err(UHorseError::DeviceNotPaired(id.clone()))
        }
    }

    async fn update_last_seen(&self, id: &DeviceId, timestamp: u64) -> Result<()> {
        let mut devices = self.devices.write().await;
        if let Some(device) = devices.get_mut(id) {
            let mut updated = device.clone();
            updated.last_seen = timestamp;
            *device = updated;
        }
        Ok(())
    }

    async fn list_devices(&self) -> Result<Vec<DeviceInfo>> {
        Ok(self.devices.read().await.values().cloned().collect())
    }
}

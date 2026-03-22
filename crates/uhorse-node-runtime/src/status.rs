//! 状态报告
//!
//! 管理节点状态上报

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use sysinfo::{Disks, System};
use tokio::sync::RwLock;
use tracing::{debug, info};
use uhorse_protocol::{LoadInfo, NodeId, NodeStatus as ProtocolNodeStatus};

/// 状态报告器
#[derive(Debug)]
pub struct StatusReporter {
    /// 节点 ID
    node_id: NodeId,

    /// 系统信息收集器
    system: Arc<RwLock<System>>,

    /// 报告间隔（秒）
    report_interval_secs: u64,

    /// 是否运行中
    running: Arc<AtomicBool>,
}

/// 心跳快照
#[derive(Debug, Clone)]
pub struct HeartbeatSnapshot {
    /// 节点状态
    pub status: ProtocolNodeStatus,
    /// 节点负载
    pub load: LoadInfo,
}

impl StatusReporter {
    /// 创建新的状态报告器
    pub fn new(node_id: NodeId) -> Self {
        Self {
            node_id,
            system: Arc::new(RwLock::new(System::new_all())),
            report_interval_secs: 10,
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// 设置报告间隔
    pub fn with_interval(mut self, secs: u64) -> Self {
        self.report_interval_secs = secs;
        self
    }

    /// 启动状态报告
    pub async fn start(&self) {
        if self.running.swap(true, std::sync::atomic::Ordering::SeqCst) {
            return; // 已经在运行
        }

        info!("Status reporter started");

        let mut interval =
            tokio::time::interval(std::time::Duration::from_secs(self.report_interval_secs));

        loop {
            interval.tick().await;

            if !self.running.load(std::sync::atomic::Ordering::SeqCst) {
                break;
            }

            if let Ok(snapshot) = self.collect_snapshot(0, 0, None).await {
                debug!("Node status: {:?}", snapshot.status);
            }
        }
    }

    /// 停止状态报告
    pub async fn stop(&self) {
        self.running
            .store(false, std::sync::atomic::Ordering::SeqCst);
        info!("Status reporter stopped");
    }

    /// 收集节点状态
    pub async fn collect_status(
        &self,
        current_tasks: usize,
        max_tasks: usize,
        network_latency_ms: Option<u64>,
    ) -> std::result::Result<ProtocolNodeStatus, Box<dyn std::error::Error>> {
        let mut sys = self.system.write().await;
        sys.refresh_all();

        let cpu_percent = sys.global_cpu_usage();
        let used_memory_mb = sys.used_memory() / (1024 * 1024);
        let disk_gb = current_disk_available_gb();

        Ok(ProtocolNodeStatus {
            node_id: self.node_id.clone(),
            online: true,
            current_tasks,
            max_tasks,
            cpu_percent,
            memory_mb: used_memory_mb,
            disk_gb,
            network_latency_ms,
            last_heartbeat: Utc::now(),
        })
    }

    /// 获取负载信息
    pub async fn get_load_info(&self, task_count: usize, latency_ms: Option<u64>) -> LoadInfo {
        self.collect_snapshot(task_count, task_count, latency_ms)
            .await
            .map(|snapshot| snapshot.load)
            .unwrap_or(LoadInfo {
                cpu_usage: 0.0,
                memory_usage: 0.0,
                task_count,
                latency_ms,
            })
    }

    /// 收集心跳快照
    pub async fn collect_snapshot(
        &self,
        current_tasks: usize,
        max_tasks: usize,
        network_latency_ms: Option<u64>,
    ) -> std::result::Result<HeartbeatSnapshot, Box<dyn std::error::Error>> {
        let mut sys = self.system.write().await;
        sys.refresh_cpu_usage();
        sys.refresh_memory();

        let cpu_percent = sys.global_cpu_usage();
        let total_memory = sys.total_memory() as f32;
        let used_memory = sys.used_memory() as f32;
        let memory_usage = if total_memory > 0.0 {
            used_memory / total_memory
        } else {
            0.0
        };
        let used_memory_mb = sys.used_memory() / (1024 * 1024);
        let disk_gb = current_disk_available_gb();
        let status = ProtocolNodeStatus {
            node_id: self.node_id.clone(),
            online: true,
            current_tasks,
            max_tasks,
            cpu_percent,
            memory_mb: used_memory_mb,
            disk_gb,
            network_latency_ms,
            last_heartbeat: Utc::now(),
        };
        let load = LoadInfo {
            cpu_usage: cpu_percent / 100.0,
            memory_usage,
            task_count: current_tasks,
            latency_ms: network_latency_ms,
        };

        Ok(HeartbeatSnapshot { status, load })
    }
}

fn current_disk_available_gb() -> f64 {
    let disks = Disks::new_with_refreshed_list();
    let available_bytes: u64 = disks.list().iter().map(|disk| disk.available_space()).sum();
    available_bytes as f64 / 1024.0 / 1024.0 / 1024.0
}

/// 节点状态（扩展）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeStatusExt {
    /// 基础状态
    #[serde(flatten)]
    pub base_status: ProtocolNodeStatus,

    /// 操作系统
    pub os_name: String,

    /// 系统版本
    pub os_version: String,

    /// 主机名
    pub hostname: String,

    /// 总内存 (GB)
    pub total_memory_gb: f64,

    /// 总磁盘 (GB)
    pub total_disk_gb: f64,

    /// 网络接口
    pub network_interfaces: Vec<NetworkInterface>,

    /// 进程数
    pub process_count: usize,

    /// 系统负载 (1/5/15 分钟)
    pub load_avg: (f32, f32, f32),
}

/// 网络接口信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInterface {
    /// 接口名称
    pub name: String,

    /// MAC 地址
    pub mac: String,

    /// IP 地址
    pub ips: Vec<String>,

    /// 发送字节数
    pub transmitted: u64,

    /// 接收字节数
    pub received: u64,
}

/// 指标收集器
#[derive(Debug, Clone, Default)]
pub struct Metrics {
    /// 总执行次数
    pub total_executions: u64,

    /// 成功次数
    pub successful_executions: u64,

    /// 失败次数
    pub failed_executions: u64,

    /// 总执行时间 (ms)
    pub total_duration_ms: u64,

    /// 平均执行时间 (ms)
    pub avg_duration_ms: f64,

    /// 总 CPU 时间 (ms)
    pub total_cpu_time_ms: u64,

    /// 总内存使用 (MB)
    pub total_memory_mb: u64,

    /// 总磁盘读取 (KB)
    pub total_disk_read_kb: u64,

    /// 总磁盘写入 (KB)
    pub total_disk_write_kb: u64,

    /// 总网络发送 (KB)
    pub total_network_sent_kb: u64,

    /// 总网络接收 (KB)
    pub total_network_recv_kb: u64,
}

impl Metrics {
    /// 记录执行
    pub fn record_execution(&mut self, success: bool, duration_ms: u64) {
        self.total_executions += 1;
        if success {
            self.successful_executions += 1;
        } else {
            self.failed_executions += 1;
        }
        self.total_duration_ms += duration_ms;
        self.avg_duration_ms = self.total_duration_ms as f64 / self.total_executions as f64;
    }

    /// 记录资源使用
    pub fn record_resources(
        &mut self,
        cpu_time_ms: u64,
        memory_mb: u64,
        disk_read_kb: u64,
        disk_write_kb: u64,
        network_sent_kb: u64,
        network_recv_kb: u64,
    ) {
        self.total_cpu_time_ms += cpu_time_ms;
        self.total_memory_mb += memory_mb;
        self.total_disk_read_kb += disk_read_kb;
        self.total_disk_write_kb += disk_write_kb;
        self.total_network_sent_kb += network_sent_kb;
        self.total_network_recv_kb += network_recv_kb;
    }

    /// 获取成功率
    pub fn success_rate(&self) -> f32 {
        if self.total_executions == 0 {
            0.0
        } else {
            self.successful_executions as f32 / self.total_executions as f32
        }
    }
}

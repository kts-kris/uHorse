//! # 任务调度器
//!
//! 支持.at、.every、cron 表达式的任务调度，带有完整的执行循环。

use uhorse_core::{Scheduler, ScheduledJob, JobId, Result, UHorseError};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration, interval};
use chrono::{DateTime, Utc};
use tracing::{debug, info, warn, error};
use std::collections::HashMap;
use super::cron::CronSchedule;

/// 任务执行器类型
pub type JobExecutor = Arc<dyn Fn(ScheduledJob) + Send + Sync>;

/// 任务调度器
pub struct JobScheduler {
    jobs: Arc<RwLock<Vec<ScheduledJob>>>,
    running: Arc<RwLock<bool>>,
    executors: Arc<RwLock<HashMap<JobId, JobExecutor>>>,
}

impl std::fmt::Debug for JobScheduler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobScheduler")
            .field("running", &*self.running.blocking_read())
            .finish()
    }
}

impl JobScheduler {
    pub fn new() -> Self {
        Self {
            jobs: Arc::new(RwLock::new(Vec::new())),
            running: Arc::new(RwLock::new(false)),
            executors: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 注册任务执行器
    pub async fn register_executor<F>(&self, id: JobId, f: F)
    where
        F: Fn(ScheduledJob) + Send + Sync + 'static,
    {
        self.executors.write().await.insert(id, Arc::new(f));
    }

    /// 计算下次执行时间
    async fn calculate_next_run(&self, job: &ScheduledJob) -> Option<DateTime<Utc>> {
        use uhorse_core::Schedule;

        match &job.schedule {
            Schedule::At { time } => {
                let dt = DateTime::from_timestamp(*time as i64, 0)?;
                if dt > Utc::now() {
                    Some(dt)
                } else {
                    None
                }
            }
            Schedule::Every { duration_secs } => {
                Some(Utc::now() + chrono::Duration::seconds(*duration_secs as i64))
            }
            Schedule::Cron { expression } => {
                let schedule = super::cron::parse_cron(expression)
                    .map_err(|e| {
                        error!("Failed to parse cron expression '{}': {}", expression, e);
                        e
                    })
                    .ok()?;
                Some(schedule.next_after(Utc::now()))
            }
        }
    }

    /// 执行单个任务
    async fn execute_job(&self, job: ScheduledJob) {
        debug!("Executing job: {}", job.id);

        // 获取执行器
        let executors = self.executors.read().await;
        if let Some(executor) = executors.get(&job.id) {
            // 在新任务中执行，避免阻塞调度循环
            let executor = Arc::clone(executor);
            tokio::spawn(async move {
                executor(job);
            });
        } else {
            warn!("No executor registered for job: {}", job.id);
        }
    }

    /// 调度循环
    async fn scheduling_loop(&self) {
        let mut ticker = interval(Duration::from_secs(1));

        while *self.running.read().await {
            ticker.tick().await;

            let jobs = self.jobs.read().await.clone();
            let now = Utc::now();

            for job in jobs {
                if let Some(next_run) = self.calculate_next_run(&job).await {
                    // 检查是否需要执行（1 秒容差）
                    if next_run <= now + chrono::Duration::seconds(1) {
                        self.execute_job(job.clone()).await;

                        // 更新任务的下一次执行时间
                        if let Ok(mut jobs) = self.jobs.try_write() {
                            if let Some(j) = jobs.iter_mut().find(|j| j.id == job.id) {
                                use uhorse_core::Schedule;
                                match &j.schedule {
                                    Schedule::At { .. } => {
                                        // 一次性任务，执行后移除
                                        jobs.retain(|j| j.id != job.id);
                                        debug!("Removed one-time job: {}", job.id);
                                    }
                                    Schedule::Every { duration_secs } => {
                                        // 更新下次执行时间
                                        j.next_run = Some((now + chrono::Duration::seconds(*duration_secs as i64)).timestamp() as u64);
                                    }
                                    Schedule::Cron { expression } => {
                                        // 重新计算下次执行时间
                                        if let Ok(schedule) = super::cron::parse_cron(expression) {
                                            j.next_run = Some(schedule.next_after(now).timestamp() as u64);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

impl Default for JobScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Scheduler for JobScheduler {
    async fn schedule_job(&mut self, job: &ScheduledJob) -> Result<()> {
        // 计算首次执行时间
        let next_run = self.calculate_next_run(job).await.map(|dt| dt.timestamp() as u64);

        let mut job = job.clone();
        job.next_run = next_run;
        let job_id = job.id.clone();
        let next_run_display = job.next_run;

        self.jobs.write().await.push(job);
        info!("Scheduled job: {} to run at {:?}", job_id, next_run_display);
        Ok(())
    }

    async fn cancel_job(&mut self, id: &JobId) -> Result<()> {
        let initial_len = self.jobs.read().await.len();
        self.jobs.write().await.retain(|j| j.id != *id);

        if self.jobs.read().await.len() < initial_len {
            info!("Cancelled job: {}", id);
            Ok(())
        } else {
            Err(UHorseError::JobNotFound(id.clone()))
        }
    }

    async fn get_job(&self, id: &JobId) -> Result<Option<ScheduledJob>> {
        Ok(self.jobs.read().await.iter().find(|j| j.id == *id).cloned())
    }

    async fn list_jobs(&self) -> Result<Vec<ScheduledJob>> {
        Ok(self.jobs.read().await.clone())
    }

    async fn start(&mut self) -> Result<()> {
        if *self.running.read().await {
            return Ok(());
        }

        info!("Starting job scheduler");
        *self.running.write().await = true;

        // 启动调度循环
        let running = self.running.clone();
        let jobs = self.jobs.clone();

        tokio::spawn(async move {
            while *running.read().await {
                sleep(Duration::from_millis(100)).await;
            }
        });

        // 启动实际的调度循环
        let scheduler = self.clone();
        tokio::spawn(async move {
            scheduler.scheduling_loop().await;
        });

        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        info!("Stopping job scheduler");
        *self.running.write().await = false;
        Ok(())
    }

    fn is_running(&self) -> bool {
        *self.running.blocking_read()
    }
}

impl Clone for JobScheduler {
    fn clone(&self) -> Self {
        Self {
            jobs: Arc::clone(&self.jobs),
            running: Arc::clone(&self.running),
            executors: Arc::clone(&self.executors),
        }
    }
}

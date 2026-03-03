//! # Cron 表达式解析
//!
//! 完整的 cron 表达式解析和计算，支持标准 5 段和 6 段格式。

use chrono::{DateTime, Datelike, Timelike, Utc};
use std::collections::HashSet;
use uhorse_core::Result;

/// Cron 表达式解析器
#[derive(Debug, Clone)]
pub struct CronParser {
    /// 是否包含秒字段（6 段格式）
    include_seconds: bool,
}

impl CronParser {
    /// 创建新的解析器（5 段格式：分 时 日 月 周）
    pub fn new() -> Self {
        Self {
            include_seconds: false,
        }
    }

    /// 创建包含秒字段的解析器（6 段格式：秒 分 时 日 月 周）
    pub fn with_seconds() -> Self {
        Self {
            include_seconds: true,
        }
    }

    /// 解析 cron 表达式
    pub fn parse(&self, expression: &str) -> Result<CronSchedule> {
        let parts: Vec<&str> = expression.split_whitespace().collect();

        let expected_len = if self.include_seconds { 6 } else { 5 };

        if parts.len() != expected_len {
            return Err(uhorse_core::UHorseError::ScheduleConflict(format!(
                "Invalid cron expression: expected {} fields, got {}",
                expected_len,
                parts.len()
            )));
        }

        let mut iter = parts.into_iter();

        // 解析各字段
        let (seconds, minutes, hours, days_of_month, months, days_of_week) = if self.include_seconds
        {
            let seconds = Self::parse_field(iter.next().unwrap(), 0, 59)?;
            let minutes = Self::parse_field(iter.next().unwrap(), 0, 59)?;
            let hours = Self::parse_field(iter.next().unwrap(), 0, 23)?;
            let days_of_month = Self::parse_field(iter.next().unwrap(), 1, 31)?;
            let months = Self::parse_field(iter.next().unwrap(), 1, 12)?;
            let days_of_week = Self::parse_field(iter.next().unwrap(), 0, 6)?;
            (
                Some(seconds),
                minutes,
                hours,
                days_of_month,
                months,
                days_of_week,
            )
        } else {
            let minutes = Self::parse_field(iter.next().unwrap(), 0, 59)?;
            let hours = Self::parse_field(iter.next().unwrap(), 0, 23)?;
            let days_of_month = Self::parse_field(iter.next().unwrap(), 1, 31)?;
            let months = Self::parse_field(iter.next().unwrap(), 1, 12)?;
            let days_of_week = Self::parse_field(iter.next().unwrap(), 0, 6)?;
            (None, minutes, hours, days_of_month, months, days_of_week)
        };

        Ok(CronSchedule {
            expression: expression.to_string(),
            include_seconds: self.include_seconds,
            seconds,
            minutes,
            hours,
            days_of_month,
            months,
            days_of_week,
        })
    }

    /// 解析单个字段
    fn parse_field(expr: &str, min: u32, max: u32) -> Result<CronField> {
        let mut values = HashSet::new();
        let mut is_range = false;
        let mut step = None;

        // 处理通配符
        if expr == "*" {
            return Ok(CronField {
                values: (min..=max).collect(),
                is_wildcard: true,
                step: None,
            });
        }

        // 处理步长 (如 */5, 1-10/2)
        if let Some(pos) = expr.find('/') {
            let base = &expr[..pos];
            step = Some(expr[pos + 1..].parse::<u32>().map_err(|_| {
                uhorse_core::UHorseError::ScheduleConflict(format!("Invalid step value: {}", expr))
            })?);

            if base == "*" {
                for i in (min..=max).step_by(step.unwrap() as usize) {
                    values.insert(i);
                }
                return Ok(CronField {
                    values,
                    is_wildcard: true,
                    step,
                });
            }

            // 处理范围步长 (如 1-10/2)
            if let Some(range_pos) = base.find('-') {
                let start: u32 = base[..range_pos].parse().map_err(|_| {
                    uhorse_core::UHorseError::ScheduleConflict(format!(
                        "Invalid range start: {}",
                        base
                    ))
                })?;
                let end: u32 = base[range_pos + 1..].parse().map_err(|_| {
                    uhorse_core::UHorseError::ScheduleConflict(format!(
                        "Invalid range end: {}",
                        base
                    ))
                })?;

                for i in (start..=end).step_by(step.unwrap() as usize) {
                    if i >= min && i <= max {
                        values.insert(i);
                    }
                }
                return Ok(CronField {
                    values,
                    is_wildcard: false,
                    step,
                });
            }
        }

        // 处理范围 (如 1-5)
        if let Some(pos) = expr.find('-') {
            is_range = true;
            let start: u32 = expr[..pos].parse().map_err(|_| {
                uhorse_core::UHorseError::ScheduleConflict(format!("Invalid range start: {}", expr))
            })?;
            let end: u32 = expr[pos + 1..].parse().map_err(|_| {
                uhorse_core::UHorseError::ScheduleConflict(format!("Invalid range end: {}", expr))
            })?;

            for i in start..=end {
                if i >= min && i <= max {
                    values.insert(i);
                }
            }

            return Ok(CronField {
                values,
                is_wildcard: false,
                step,
            });
        }

        // 处理列表 (如 1,2,3 或 1,3-5,7)
        for part in expr.split(',') {
            if let Some(range_pos) = part.find('-') {
                let start: u32 = part[..range_pos].parse().map_err(|_| {
                    uhorse_core::UHorseError::ScheduleConflict(format!(
                        "Invalid list range start: {}",
                        part
                    ))
                })?;
                let end: u32 = part[range_pos + 1..].parse().map_err(|_| {
                    uhorse_core::UHorseError::ScheduleConflict(format!(
                        "Invalid list range end: {}",
                        part
                    ))
                })?;

                for i in start..=end {
                    if i >= min && i <= max {
                        values.insert(i);
                    }
                }
            } else {
                let val: u32 = part.parse().map_err(|_| {
                    uhorse_core::UHorseError::ScheduleConflict(format!("Invalid value: {}", part))
                })?;

                if val < min || val > max {
                    return Err(uhorse_core::UHorseError::ScheduleConflict(format!(
                        "Value {} out of range ({}-{})",
                        val, min, max
                    )));
                }
                values.insert(val);
            }
        }

        Ok(CronField {
            values,
            is_wildcard: false,
            step,
        })
    }
}

impl Default for CronParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Cron 字段
#[derive(Debug, Clone)]
pub struct CronField {
    /// 允许的值集合
    pub values: HashSet<u32>,
    /// 是否为通配符
    pub is_wildcard: bool,
    /// 步长值
    pub step: Option<u32>,
}

/// Cron 调度表
#[derive(Debug, Clone)]
pub struct CronSchedule {
    pub expression: String,
    pub include_seconds: bool,
    pub seconds: Option<CronField>,
    pub minutes: CronField,
    pub hours: CronField,
    pub days_of_month: CronField,
    pub months: CronField,
    pub days_of_week: CronField,
}

impl CronSchedule {
    /// 计算下一次执行时间
    pub fn next_after(&self, after: DateTime<Utc>) -> DateTime<Utc> {
        let mut current = after + chrono::Duration::seconds(1);

        // 最多向前查找 4 年（覆盖闰年情况）
        for _ in 0..365 * 4 * 24 * 60 {
            if self.matches(&current) {
                return current;
            }
            current += chrono::Duration::seconds(60);
        }

        // 如果找不到，返回远期时间
        after + chrono::Duration::days(365 * 4)
    }

    /// 检查给定时间是否匹配 cron 表达式
    pub fn matches(&self, dt: &DateTime<Utc>) -> bool {
        // 检查秒
        if let Some(ref seconds) = self.seconds {
            if !seconds.values.contains(&dt.second()) {
                return false;
            }
        }

        // 检查分
        if !self.minutes.values.contains(&dt.minute()) {
            return false;
        }

        // 检查时
        if !self.hours.values.contains(&dt.hour()) {
            return false;
        }

        // 检查月
        if !self.months.values.contains(&dt.month()) {
            return false;
        }

        // 检查日和星期（逻辑或关系）
        let day_matches = self.days_of_month.values.contains(&dt.day());
        let weekday_matches = self
            .days_of_week
            .values
            .contains(&(dt.weekday().num_days_from_monday()));

        // 特殊处理：如果都是 *，则每天都匹配
        let all_wildcard = self.days_of_month.is_wildcard && self.days_of_week.is_wildcard;

        if !all_wildcard && !day_matches && !weekday_matches {
            return false;
        }

        true
    }

    /// 获取描述性文本
    pub fn description(&self) -> String {
        format!("Cron schedule: {}", self.expression)
    }
}

/// 便捷函数：解析标准 cron 表达式
pub fn parse_cron(expression: &str) -> Result<CronSchedule> {
    CronParser::new().parse(expression)
}

/// 便捷函数：解析包含秒的 cron 表达式
pub fn parse_cron_with_seconds(expression: &str) -> Result<CronSchedule> {
    CronParser::with_seconds().parse(expression)
}

/// 常用 cron 表达式
pub mod presets {
    use super::*;

    /// 每分钟
    pub fn every_minute() -> &'static str {
        "* * * * *"
    }

    /// 每小时
    pub fn every_hour() -> &'static str {
        "0 * * * *"
    }

    /// 每天午夜
    pub fn daily_midnight() -> &'static str {
        "0 0 * * *"
    }

    /// 每天上午 9 点
    pub fn daily_9am() -> &'static str {
        "0 9 * * *"
    }

    /// 每周一上午 9 点
    pub fn weekly_monday_9am() -> &'static str {
        "0 9 * * 1"
    }

    /// 每月第一天午夜
    pub fn monthly_first_midnight() -> &'static str {
        "0 0 1 * *"
    }

    /// 每 5 分钟
    pub fn every_5_minutes() -> &'static str {
        "*/5 * * * *"
    }

    /// 每 15 分钟
    pub fn every_15_minutes() -> &'static str {
        "*/15 * * * *"
    }

    /// 每 30 分钟
    pub fn every_30_minutes() -> &'static str {
        "*/30 * * * *"
    }

    /// 工作日上午 9 点
    pub fn weekdays_9am() -> &'static str {
        "0 9 * * 1-5"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple() {
        let schedule = parse_cron("0 9 * * *").unwrap();
        assert_eq!(schedule.expression, "0 9 * * *");
    }

    #[test]
    fn test_parse_with_seconds() {
        let schedule = parse_cron_with_seconds("30 */5 9-17 * * 1-5").unwrap();
        assert!(schedule.seconds.is_some());
        assert!(schedule.include_seconds);
    }

    #[test]
    fn test_matches_exact() {
        let schedule = parse_cron("30 14 * * *").unwrap();
        let dt = DateTime::parse_from_rfc3339("2024-01-15T14:30:00Z")
            .unwrap()
            .with_timezone(&Utc);
        assert!(schedule.matches(&dt));
    }

    #[test]
    fn test_matches_every_hour() {
        let schedule = parse_cron("0 * * * *").unwrap();
        let dt = DateTime::parse_from_rfc3339("2024-01-15T09:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        assert!(schedule.matches(&dt));
    }

    #[test]
    fn test_next_after() {
        let schedule = parse_cron("0 9 * * *").unwrap();
        let dt = DateTime::parse_from_rfc3339("2024-01-15T08:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let next = schedule.next_after(dt);
        assert_eq!(next.hour(), 9);
        assert_eq!(next.minute(), 0);
    }
}

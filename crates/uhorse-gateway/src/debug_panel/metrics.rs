//! Metrics collection for debug panel

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Time-series data point
#[derive(Debug, Clone, serde::Serialize)]
pub struct DataPoint {
    pub timestamp: i64,
    pub value: f64,
}

/// Rolling window metrics collector
#[derive(Debug)]
pub struct MetricsCollector {
    /// Window size
    window_size: Duration,
    /// Data points
    points: VecDeque<TimedPoint>,
}

#[derive(Debug)]
struct TimedPoint {
    timestamp: Instant,
    value: f64,
}

impl MetricsCollector {
    /// Create a new collector with given window size
    pub fn new(window_size: Duration) -> Self {
        Self {
            window_size,
            points: VecDeque::new(),
        }
    }

    /// Add a data point
    pub fn record(&mut self, value: f64) {
        let now = Instant::now();
        let cutoff = now - self.window_size;

        // Remove old points
        while let Some(front) = self.points.front() {
            if front.timestamp < cutoff {
                self.points.pop_front();
            } else {
                break;
            }
        }

        // Add new point
        self.points.push_back(TimedPoint {
            timestamp: now,
            value,
        });
    }

    /// Get average value
    pub fn average(&self) -> Option<f64> {
        if self.points.is_empty() {
            return None;
        }

        let sum: f64 = self.points.iter().map(|p| p.value).sum();
        Some(sum / self.points.len() as f64)
    }

    /// Get min value
    pub fn min(&self) -> Option<f64> {
        self.points.iter().map(|p| p.value).reduce(f64::min)
    }

    /// Get max value
    pub fn max(&self) -> Option<f64> {
        self.points.iter().map(|p| p.value).reduce(f64::max)
    }

    /// Get count
    pub fn count(&self) -> usize {
        self.points.len()
    }

    /// Get percentile (0-100)
    pub fn percentile(&self, p: f64) -> Option<f64> {
        if self.points.is_empty() {
            return None;
        }

        let mut values: Vec<f64> = self.points.iter().map(|p| p.value).collect();
        values.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let idx = ((p / 100.0) * (values.len() - 1) as f64).round() as usize;
        Some(values[idx])
    }

    /// Get data points for charting
    pub fn to_data_points(&self) -> Vec<DataPoint> {
        let now = chrono::Utc::now().timestamp();

        self.points
            .iter()
            .map(|p| {
                let elapsed = Instant::now().duration_since(p.timestamp);
                DataPoint {
                    timestamp: now - elapsed.as_secs() as i64,
                    value: p.value,
                }
            })
            .collect()
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new(Duration::from_secs(3600)) // 1 hour window
    }
}

/// Performance statistics
#[derive(Debug, Default, serde::Serialize)]
pub struct PerformanceStats {
    /// Response time collector
    pub response_times: MetricsCollector,
    /// Request rate collector
    pub request_rate: MetricsCollector,
    /// Error rate collector
    pub error_rate: MetricsCollector,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_collector() {
        let mut collector = MetricsCollector::new(Duration::from_secs(60));

        collector.record(100.0);
        collector.record(200.0);
        collector.record(300.0);

        assert_eq!(collector.count(), 3);
        assert_eq!(collector.average(), Some(200.0));
        assert_eq!(collector.min(), Some(100.0));
        assert_eq!(collector.max(), Some(300.0));
    }

    #[test]
    fn test_percentile() {
        let mut collector = MetricsCollector::new(Duration::from_secs(60));

        for i in 1..=100 {
            collector.record(i as f64);
        }

        assert_eq!(collector.percentile(50.0), Some(50.0));
        assert_eq!(collector.percentile(95.0), Some(95.0));
        assert_eq!(collector.percentile(99.0), Some(99.0));
    }
}

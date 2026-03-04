import React from 'react';
import { useQuery } from '@tanstack/react-query';
import { Card, Row, Col, Statistic, Spin, Alert } from 'antd';
import {
  RobotOutlined,
  MessageOutlined,
  ApiOutlined,
  ClockCircleOutlined,
} from '@ant-design/icons';
import { systemService } from '../services/system';

const Dashboard: React.FC = () => {
  const { data: info, isLoading: infoLoading, error: infoError } = useQuery({
    queryKey: ['systemInfo'],
    queryFn: systemService.getInfo,
  });

  const { data: metrics, isLoading: metricsLoading } = useQuery({
    queryKey: ['systemMetrics'],
    queryFn: systemService.getMetrics,
    refetchInterval: 5000,
  });

  if (infoLoading) {
    return <Spin size="large" />;
  }

  if (infoError) {
    return <Alert type="error" message="加载系统信息失败" />;
  }

  return (
    <div>
      <h2 style={{ marginBottom: 24 }}>系统概览</h2>

      <Row gutter={[16, 16]}>
        <Col xs={24} sm={12} md={6}>
          <Card>
            <Statistic
              title="Agent 数量"
              value={info?.agents_count || 0}
              prefix={<RobotOutlined />}
            />
          </Card>
        </Col>
        <Col xs={24} sm={12} md={6}>
          <Card>
            <Statistic
              title="活跃 Session"
              value={info?.active_sessions || 0}
              prefix={<MessageOutlined />}
            />
          </Card>
        </Col>
        <Col xs={24} sm={12} md={6}>
          <Card>
            <Statistic
              title="通道数量"
              value={info?.channels_count || 0}
              prefix={<ApiOutlined />}
            />
          </Card>
        </Col>
        <Col xs={24} sm={12} md={6}>
          <Card>
            <Statistic
              title="运行时间"
              value={formatUptime(info?.uptime_secs || 0)}
              prefix={<ClockCircleOutlined />}
            />
          </Card>
        </Col>
      </Row>

      <h3 style={{ margin: '24px 0 16px' }}>系统指标</h3>
      <Row gutter={[16, 16]}>
        <Col xs={24} sm={12} md={6}>
          <Card loading={metricsLoading}>
            <Statistic
              title="今日消息"
              value={metrics?.messages_today || 0}
            />
          </Card>
        </Col>
        <Col xs={24} sm={12} md={6}>
          <Card loading={metricsLoading}>
            <Statistic
              title="总消息数"
              value={metrics?.total_messages || 0}
            />
          </Card>
        </Col>
        <Col xs={24} sm={12} md={6}>
          <Card loading={metricsLoading}>
            <Statistic
              title="平均响应时间"
              value={metrics?.avg_response_time_ms || 0}
              suffix="ms"
              precision={2}
            />
          </Card>
        </Col>
        <Col xs={24} sm={12} md={6}>
          <Card loading={metricsLoading}>
            <Statistic
              title="错误率"
              value={
                metrics?.total_requests
                  ? ((metrics.total_errors / metrics.total_requests) * 100).toFixed(2)
                  : 0
              }
              suffix="%"
            />
          </Card>
        </Col>
      </Row>

      <Card style={{ marginTop: 24 }}>
        <h4>系统信息</h4>
        <p>
          <strong>版本:</strong> {info?.version}
        </p>
        <p>
          <strong>Rust 版本:</strong> {info?.rust_version}
        </p>
        <p>
          <strong>系统名称:</strong> {info?.name}
        </p>
      </Card>
    </div>
  );
};

function formatUptime(secs: number): string {
  const days = Math.floor(secs / 86400);
  const hours = Math.floor((secs % 86400) / 3600);
  const minutes = Math.floor((secs % 3600) / 60);

  if (days > 0) return `${days}天 ${hours}小时`;
  if (hours > 0) return `${hours}小时 ${minutes}分钟`;
  return `${minutes}分钟`;
}

export default Dashboard;

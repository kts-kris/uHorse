import React, { useMemo } from 'react';
import { useQuery } from '@tanstack/react-query';
import {
  Alert,
  Card,
  Col,
  Descriptions,
  List,
  Progress,
  Row,
  Space,
  Spin,
  Statistic,
  Tag,
  Typography,
} from 'antd';
import {
  ClockCircleOutlined,
  MessageOutlined,
  RobotOutlined,
  ApiOutlined,
  CheckCircleOutlined,
} from '@ant-design/icons';

import { agentService, sessionService } from '../services/agents';
import { systemService } from '../services/system';

const Dashboard: React.FC = () => {
  const {
    data: stats,
    isLoading: statsLoading,
    error: statsError,
    isFetching: isStatsFetching,
  } = useQuery({
    queryKey: ['hub-stats'],
    queryFn: systemService.getStats,
    refetchInterval: 5000,
  });

  const {
    data: agents = [],
    isLoading: agentsLoading,
    error: agentsError,
  } = useQuery({
    queryKey: ['agents-runtime'],
    queryFn: agentService.list,
  });

  const {
    data: sessions = [],
    isLoading: sessionsLoading,
    error: sessionsError,
  } = useQuery({
    queryKey: ['sessions-runtime'],
    queryFn: sessionService.list,
  });

  const {
    data: nodes = [],
    isLoading: nodesLoading,
    error: nodesError,
  } = useQuery({
    queryKey: ['runtime-nodes'],
    queryFn: systemService.getNodes,
    refetchInterval: 5000,
  });

  const {
    data: tasks = [],
    isLoading: tasksLoading,
    error: tasksError,
  } = useQuery({
    queryKey: ['runtime-tasks'],
    queryFn: systemService.getTasks,
    refetchInterval: 5000,
  });

  const loading = statsLoading || agentsLoading || sessionsLoading || nodesLoading || tasksLoading;
  const errors = [statsError, agentsError, sessionsError, nodesError, tasksError].filter(Boolean);

  const runtimeStats = useMemo(() => {
    const runningTasks = tasks.filter((task) => task.status.toLowerCase() === 'running').length;
    const completedTasks = tasks.filter((task) => task.status.toLowerCase() === 'completed').length;
    return {
      agentCount: agents.length,
      activeSessions: sessions.length,
      onlineNodes: stats?.nodes.online_nodes || 0,
      runningTasks,
      completedTasks,
    };
  }, [agents, sessions, stats, tasks]);

  if (loading && !stats) {
    return (
      <div style={{ textAlign: 'center', padding: 48 }}>
        <Spin size="large" />
      </div>
    );
  }

  return (
    <Space direction="vertical" size={16} style={{ width: '100%' }}>
      {errors.map((error, index) => (
        <Alert
          key={index}
          type="error"
          showIcon
          message="加载运行时数据失败"
          description={error instanceof Error ? error.message : '未知错误'}
        />
      ))}

      <Row gutter={[16, 16]}>
        <Col xs={24} sm={12} md={6}>
          <Card loading={loading && !stats}>
            <Statistic title="Agent 数量" value={runtimeStats.agentCount} prefix={<RobotOutlined />} />
          </Card>
        </Col>
        <Col xs={24} sm={12} md={6}>
          <Card loading={loading && !stats}>
            <Statistic title="活跃 Session" value={runtimeStats.activeSessions} prefix={<MessageOutlined />} />
          </Card>
        </Col>
        <Col xs={24} sm={12} md={6}>
          <Card loading={loading && !stats}>
            <Statistic title="在线节点" value={runtimeStats.onlineNodes} prefix={<ApiOutlined />} />
          </Card>
        </Col>
        <Col xs={24} sm={12} md={6}>
          <Card loading={loading && !stats}>
            <Statistic
              title="运行时间"
              value={formatUptime(stats?.uptime_secs || 0)}
              prefix={<ClockCircleOutlined />}
            />
          </Card>
        </Col>
      </Row>

      <Row gutter={[16, 16]}>
        <Col xs={24} lg={12}>
          <Card title="Hub 概览" extra={isStatsFetching ? '刷新中' : undefined}>
            <Descriptions bordered size="small" column={1}>
              <Descriptions.Item label="Hub ID">{stats?.hub_id || '-'}</Descriptions.Item>
              <Descriptions.Item label="更新时间">
                {stats?.updated_at ? formatDateTime(stats.updated_at) : '-'}
              </Descriptions.Item>
              <Descriptions.Item label="节点总数">{stats?.nodes.total_nodes || 0}</Descriptions.Item>
              <Descriptions.Item label="在线节点">{stats?.nodes.online_nodes || 0}</Descriptions.Item>
              <Descriptions.Item label="离线节点">{stats?.nodes.offline_nodes || 0}</Descriptions.Item>
              <Descriptions.Item label="忙碌节点">{stats?.nodes.busy_nodes || 0}</Descriptions.Item>
              <Descriptions.Item label="待调度任务">{stats?.scheduler.pending_tasks || 0}</Descriptions.Item>
              <Descriptions.Item label="运行中任务">{stats?.scheduler.running_tasks || 0}</Descriptions.Item>
              <Descriptions.Item label="已完成任务">{stats?.scheduler.completed_tasks || 0}</Descriptions.Item>
              <Descriptions.Item label="失败任务">{stats?.scheduler.failed_tasks || 0}</Descriptions.Item>
            </Descriptions>
          </Card>
        </Col>

        <Col xs={24} lg={12}>
          <Card title="任务概览">
            <Row gutter={[16, 16]}>
              <Col span={12}>
                <Statistic
                  title="运行中"
                  value={runtimeStats.runningTasks}
                  prefix={<ClockCircleOutlined />}
                />
              </Col>
              <Col span={12}>
                <Statistic
                  title="已完成"
                  value={runtimeStats.completedTasks}
                  prefix={<CheckCircleOutlined />}
                />
              </Col>
            </Row>
            <List
              style={{ marginTop: 16 }}
              size="small"
              dataSource={tasks.slice(0, 5)}
              locale={{ emptyText: '暂无任务' }}
              renderItem={(task) => (
                <List.Item>
                  <Space direction="vertical" size={2} style={{ width: '100%' }}>
                    <Space>
                      <Typography.Text code>{task.task_id}</Typography.Text>
                      <Tag color={taskStatusColor(task.status)}>{task.status}</Tag>
                    </Space>
                    <Typography.Text type="secondary">
                      {task.command_type} · {task.priority}
                      {task.started_at ? ` · ${formatDateTime(task.started_at)}` : ''}
                    </Typography.Text>
                  </Space>
                </List.Item>
              )}
            />
          </Card>
        </Col>
      </Row>

      <Card title="节点运行时">
        <List
          dataSource={nodes}
          locale={{ emptyText: '暂无在线节点数据' }}
          renderItem={(node) => (
            <List.Item>
              <Row gutter={[16, 16]} style={{ width: '100%' }}>
                <Col xs={24} md={8}>
                  <Space direction="vertical" size={4}>
                    <Space>
                      <Typography.Text strong>{node.name}</Typography.Text>
                      <Tag color={nodeStateColor(node.state)}>{node.state}</Tag>
                    </Space>
                    <Typography.Text code>{node.node_id}</Typography.Text>
                    <Typography.Text type="secondary">{node.workspace.path}</Typography.Text>
                  </Space>
                </Col>
                <Col xs={24} md={8}>
                  <Space direction="vertical" size={8} style={{ width: '100%' }}>
                    <div>
                      <Typography.Text type="secondary">CPU</Typography.Text>
                      <Progress percent={toPercent(node.load.cpu_usage)} size="small" />
                    </div>
                    <div>
                      <Typography.Text type="secondary">内存</Typography.Text>
                      <Progress percent={toPercent(node.load.memory_usage)} size="small" status="active" />
                    </div>
                  </Space>
                </Col>
                <Col xs={24} md={8}>
                  <Descriptions size="small" column={1}>
                    <Descriptions.Item label="当前任务">{node.current_tasks}</Descriptions.Item>
                    <Descriptions.Item label="累计完成">{node.completed_tasks}</Descriptions.Item>
                    <Descriptions.Item label="累计失败">{node.failed_tasks}</Descriptions.Item>
                    <Descriptions.Item label="最后心跳">
                      {formatDateTime(node.last_heartbeat)}
                    </Descriptions.Item>
                  </Descriptions>
                </Col>
              </Row>
            </List.Item>
          )}
        />
      </Card>
    </Space>
  );
};

function formatUptime(secs: number): string {
  const days = Math.floor(secs / 86400);
  const hours = Math.floor((secs % 86400) / 3600);
  const minutes = Math.floor((secs % 3600) / 60);

  if (days > 0) return `${days} 天 ${hours} 小时`;
  if (hours > 0) return `${hours} 小时 ${minutes} 分钟`;
  return `${minutes} 分钟`;
}

function formatDateTime(value: string): string {
  const timestamp = Date.parse(value);
  if (Number.isNaN(timestamp)) {
    return value;
  }
  return new Date(timestamp).toLocaleString();
}

function toPercent(value: number): number {
  return Math.max(0, Math.min(100, Math.round(value * 100)));
}

function taskStatusColor(status: string): string {
  switch (status.toLowerCase()) {
    case 'running':
      return 'processing';
    case 'completed':
      return 'success';
    case 'failed':
    case 'timeout':
      return 'error';
    case 'cancelled':
      return 'default';
    default:
      return 'blue';
  }
}

function nodeStateColor(state: string): string {
  switch (state.toLowerCase()) {
    case 'online':
      return 'green';
    case 'busy':
      return 'orange';
    case 'offline':
      return 'default';
    case 'maintenance':
      return 'purple';
    default:
      return 'blue';
  }
}

export default Dashboard;

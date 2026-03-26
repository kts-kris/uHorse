import React from 'react';
import { useQuery } from '@tanstack/react-query';
import {
  Alert,
  Card,
  Col,
  Descriptions,
  List,
  Row,
  Space,
  Spin,
  Statistic,
  Tag,
  Typography,
} from 'antd';
import {
  ClockCircleOutlined,
  DatabaseOutlined,
  SafetyOutlined,
  SettingOutlined,
} from '@ant-design/icons';

import { skillService } from '../services/agents';
import { systemService } from '../services/system';

const sourceLayerColorMap: Record<string, string> = {
  global: 'default',
  tenant: 'purple',
  user: 'cyan',
};

const Settings: React.FC = () => {
  const {
    data: stats,
    isLoading: statsLoading,
    error: statsError,
  } = useQuery({
    queryKey: ['hub-stats'],
    queryFn: systemService.getStats,
    refetchInterval: 5000,
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
    data: skills = [],
    isLoading: skillsLoading,
    error: skillsError,
  } = useQuery({
    queryKey: ['skills-runtime'],
    queryFn: skillService.list,
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

  const loading = statsLoading || nodesLoading || skillsLoading || tasksLoading;

  if (loading && !stats) {
    return (
      <div style={{ textAlign: 'center', padding: 48 }}>
        <Spin size="large" />
      </div>
    );
  }

  return (
    <Space direction="vertical" size={16} style={{ width: '100%' }}>
      <Alert
        type="info"
        showIcon
        message="当前页面展示系统运行时与配置摘要，不提供在线写配置。"
        description="如果需要调整 Hub / Agent / Skill 配置，请修改部署配置、skill.toml 或对应运行目录后重启服务。"
      />

      {[statsError, nodesError, skillsError, tasksError]
        .filter(Boolean)
        .map((error, index) => (
          <Alert
            key={index}
            type="error"
            showIcon
            message="加载运行时信息失败"
            description={error instanceof Error ? error.message : '未知错误'}
          />
        ))}

      <Row gutter={[16, 16]}>
        <Col xs={24} sm={12} md={6}>
          <Card>
            <Statistic title="Hub 运行时间" value={formatUptime(stats?.uptime_secs || 0)} prefix={<ClockCircleOutlined />} />
          </Card>
        </Col>
        <Col xs={24} sm={12} md={6}>
          <Card>
            <Statistic title="技能数量" value={skills.length} prefix={<DatabaseOutlined />} />
          </Card>
        </Col>
        <Col xs={24} sm={12} md={6}>
          <Card>
            <Statistic title="节点数量" value={stats?.nodes.total_nodes || nodes.length} prefix={<SettingOutlined />} />
          </Card>
        </Col>
        <Col xs={24} sm={12} md={6}>
          <Card>
            <Statistic title="任务积压" value={stats?.scheduler.pending_tasks || 0} prefix={<SafetyOutlined />} />
          </Card>
        </Col>
      </Row>

      <Row gutter={[16, 16]}>
        <Col xs={24} lg={12}>
          <Card title="Hub 运行参数">
            <Descriptions bordered size="small" column={1}>
              <Descriptions.Item label="Hub ID">{stats?.hub_id || '-'}</Descriptions.Item>
              <Descriptions.Item label="更新时间">
                {stats?.updated_at ? formatDateTime(stats.updated_at) : '-'}
              </Descriptions.Item>
              <Descriptions.Item label="节点调度">
                在线 {stats?.nodes.online_nodes || 0} / 总计 {stats?.nodes.total_nodes || 0}
              </Descriptions.Item>
              <Descriptions.Item label="任务队列">
                待调度 {stats?.scheduler.pending_tasks || 0}，运行中 {stats?.scheduler.running_tasks || 0}
              </Descriptions.Item>
            </Descriptions>
          </Card>
        </Col>

        <Col xs={24} lg={12}>
          <Card title="Skill 运行配置">
            <List
              dataSource={skills}
              locale={{ emptyText: '暂无 Skill' }}
              renderItem={(skill) => (
                <List.Item
                  key={`${skill.name}:${skill.source_layer}:${skill.source_scope ?? 'global'}`}
                >
                  <Descriptions size="small" column={1} style={{ width: '100%' }}>
                    <Descriptions.Item label="Skill">
                      <Space>
                        <Typography.Text strong>{skill.name}</Typography.Text>
                        <Tag color={skill.enabled ? 'green' : 'default'}>
                          {skill.enabled ? '启用' : '禁用'}
                        </Tag>
                      </Space>
                    </Descriptions.Item>
                    <Descriptions.Item label="执行方式">{skill.execution_mode}</Descriptions.Item>
                    <Descriptions.Item label="来源">
                      <Tag color={sourceLayerColorMap[skill.source_layer] ?? 'default'}>
                        {skill.source_scope
                          ? `${skill.source_layer} · ${skill.source_scope}`
                          : skill.source_layer}
                      </Tag>
                    </Descriptions.Item>
                    <Descriptions.Item label="超时">{skill.timeout_secs}s</Descriptions.Item>
                    <Descriptions.Item label="命令来源">
                      {skill.executable || '内置 / dummy'}
                    </Descriptions.Item>
                  </Descriptions>
                </List.Item>
              )}
            />
          </Card>
        </Col>
      </Row>

      <Card title="节点工作空间">
        <List
          dataSource={nodes}
          locale={{ emptyText: '暂无节点工作空间数据' }}
          renderItem={(node) => (
            <List.Item key={node.node_id}>
              <Descriptions bordered size="small" column={1} style={{ width: '100%' }}>
                <Descriptions.Item label="节点">
                  <Space>
                    <Typography.Text strong>{node.name}</Typography.Text>
                    <Tag>{node.state}</Tag>
                  </Space>
                </Descriptions.Item>
                <Descriptions.Item label="Workspace 路径">{node.workspace.path}</Descriptions.Item>
                <Descriptions.Item label="只读模式">
                  {node.workspace.read_only ? '是' : '否'}
                </Descriptions.Item>
                <Descriptions.Item label="允许模式">
                  {node.workspace.allowed_patterns.join(', ') || '-'}
                </Descriptions.Item>
                <Descriptions.Item label="拒绝模式">
                  {node.workspace.denied_patterns.join(', ') || '-'}
                </Descriptions.Item>
              </Descriptions>
            </List.Item>
          )}
        />
      </Card>

      <Card title="最近任务状态">
        <List
          dataSource={tasks.slice(0, 8)}
          locale={{ emptyText: '暂无任务记录' }}
          renderItem={(task) => (
            <List.Item key={task.task_id}>
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

export default Settings;

import { Alert, Button, Card, Col, Descriptions, Row, Space, Spin, Statistic, Tag, Typography } from 'antd';
import {
  ApiOutlined,
  ClockCircleOutlined,
  HistoryOutlined,
  PauseCircleOutlined,
  PlayCircleOutlined,
  SafetyOutlined,
} from '@ant-design/icons';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { desktopApi } from '../services/desktopApi';
import { useNodeStatus } from '../hooks/useNodeStatus';
import type { DesktopLifecycleState } from '../types/desktop';

const Dashboard: React.FC = () => {
  const queryClient = useQueryClient();
  const { data: status, isLoading, error, isFetching } = useNodeStatus();

  const startMutation = useMutation({
    mutationFn: desktopApi.startNode,
    onSuccess: (next) => {
      queryClient.setQueryData(['desktop-runtime-status'], next);
      void queryClient.invalidateQueries({ queryKey: ['desktop-workspace-status'] });
      void queryClient.invalidateQueries({ queryKey: ['desktop-version-summary'] });
    },
  });

  const stopMutation = useMutation({
    mutationFn: desktopApi.stopNode,
    onSuccess: (next) => {
      queryClient.setQueryData(['desktop-runtime-status'], next);
    },
  });

  if (isLoading && !status) {
    return (
      <div style={{ textAlign: 'center', padding: 48 }}>
        <Spin size="large" />
      </div>
    );
  }

  if (error instanceof Error && !status) {
    return <Alert type="error" showIcon message="加载运行时状态失败" description={error.message} />;
  }

  if (!status) {
    return <Alert type="warning" showIcon message="暂无运行时状态" />;
  }

  const busy = startMutation.isPending || stopMutation.isPending;
  const canStart = status.lifecycle_state === 'stopped' || status.lifecycle_state === 'failed';
  const canStop = status.lifecycle_state === 'running' || status.lifecycle_state === 'starting';

  return (
    <Space direction="vertical" size={16} style={{ width: '100%' }}>
      {error instanceof Error ? (
        <Alert type="error" showIcon message="刷新运行时状态失败" description={error.message} />
      ) : null}
      {startMutation.error instanceof Error ? (
        <Alert type="error" showIcon message="启动失败" description={startMutation.error.message} />
      ) : null}
      {stopMutation.error instanceof Error ? (
        <Alert type="error" showIcon message="停止失败" description={stopMutation.error.message} />
      ) : null}

      <Space style={{ width: '100%', justifyContent: 'space-between' }} wrap>
        <Typography.Title level={4} style={{ margin: 0 }}>
          本地 Node 仪表盘
        </Typography.Title>
        <Space>
          <Tag color={lifecycleColor(status.lifecycle_state)}>{status.lifecycle_state}</Tag>
          <Button
            type="primary"
            icon={<PlayCircleOutlined />}
            onClick={() => startMutation.mutate()}
            loading={startMutation.isPending}
            disabled={!canStart || busy}
          >
            启动
          </Button>
          <Button
            icon={<PauseCircleOutlined />}
            onClick={() => stopMutation.mutate()}
            loading={stopMutation.isPending}
            disabled={!canStop || busy}
          >
            停止
          </Button>
        </Space>
      </Space>

      <Row gutter={[16, 16]}>
        <Col xs={24} md={6}>
          <Card>
            <Statistic title="连接状态" value={status.connection_state} prefix={<ApiOutlined />} />
          </Card>
        </Col>
        <Col xs={24} md={6}>
          <Card>
            <Statistic title="运行任务" value={status.running_tasks} prefix={<ClockCircleOutlined />} />
          </Card>
        </Col>
        <Col xs={24} md={6}>
          <Card>
            <Statistic title="待审批" value={status.pending_approvals} prefix={<SafetyOutlined />} />
          </Card>
        </Col>
        <Col xs={24} md={6}>
          <Card>
            <Statistic
              title="最近检查点"
              value={status.latest_checkpoint?.message || '-'}
              prefix={<HistoryOutlined />}
            />
          </Card>
        </Col>
      </Row>

      <Row gutter={[16, 16]}>
        <Col xs={24} lg={12}>
          <Card title="节点摘要" extra={isFetching ? '刷新中' : undefined}>
            <Descriptions bordered size="small" column={1}>
              <Descriptions.Item label="节点名称">{status.name}</Descriptions.Item>
              <Descriptions.Item label="节点 ID">{status.node_id || '-'}</Descriptions.Item>
              <Descriptions.Item label="工作区路径">{status.workspace_path}</Descriptions.Item>
              <Descriptions.Item label="Hub 地址">{status.hub_url}</Descriptions.Item>
              <Descriptions.Item label="生命周期">
                <Tag color={lifecycleColor(status.lifecycle_state)}>{status.lifecycle_state}</Tag>
              </Descriptions.Item>
              <Descriptions.Item label="Hub 连接">
                <Tag color={connectionColor(status.connection_state)}>{status.connection_state}</Tag>
              </Descriptions.Item>
              <Descriptions.Item label="最大并发">{status.max_concurrent_tasks}</Descriptions.Item>
              <Descriptions.Item label="最近错误">{status.recent_error || '-'}</Descriptions.Item>
            </Descriptions>
          </Card>
        </Col>

        <Col xs={24} lg={12}>
          <Card title="执行指标">
            <Descriptions bordered size="small" column={1}>
              <Descriptions.Item label="总执行次数">{status.metrics.total_executions}</Descriptions.Item>
              <Descriptions.Item label="成功次数">{status.metrics.successful_executions}</Descriptions.Item>
              <Descriptions.Item label="失败次数">{status.metrics.failed_executions}</Descriptions.Item>
              <Descriptions.Item label="平均耗时">
                {status.metrics.avg_duration_ms.toFixed(2)} ms
              </Descriptions.Item>
              <Descriptions.Item label="成功率">
                {(status.metrics.success_rate * 100).toFixed(1)}%
              </Descriptions.Item>
              <Descriptions.Item label="CPU">
                {status.heartbeat ? `${status.heartbeat.cpu_percent.toFixed(1)}%` : '-'}
              </Descriptions.Item>
              <Descriptions.Item label="内存">
                {status.heartbeat ? `${status.heartbeat.memory_mb} MB` : '-'}
              </Descriptions.Item>
              <Descriptions.Item label="磁盘">
                {status.heartbeat ? `${status.heartbeat.disk_gb.toFixed(2)} GB` : '-'}
              </Descriptions.Item>
              <Descriptions.Item label="网络延迟">
                {status.heartbeat?.network_latency_ms != null
                  ? `${status.heartbeat.network_latency_ms} ms`
                  : '-'}
              </Descriptions.Item>
              <Descriptions.Item label="最后心跳">
                {formatDateTime(status.heartbeat?.last_heartbeat)}
              </Descriptions.Item>
            </Descriptions>
          </Card>
        </Col>
      </Row>
    </Space>
  );
};

function lifecycleColor(state: DesktopLifecycleState): string {
  switch (state) {
    case 'running':
      return 'success';
    case 'starting':
    case 'stopping':
      return 'processing';
    case 'failed':
      return 'error';
    default:
      return 'default';
  }
}

function connectionColor(state: string): string {
  if (state.startsWith('authenticated') || state.startsWith('connected')) {
    return 'success';
  }
  if (state.startsWith('connecting') || state.startsWith('reconnecting') || state.startsWith('authenticating')) {
    return 'processing';
  }
  if (state.startsWith('failed')) {
    return 'error';
  }
  return 'default';
}

function formatDateTime(value?: string | null): string {
  if (!value) {
    return '-';
  }

  const timestamp = Date.parse(value);
  if (Number.isNaN(timestamp)) {
    return value;
  }

  return new Date(timestamp).toLocaleString();
}

export default Dashboard;

import { Alert, Card, Descriptions, List, Space, Spin, Tag, Typography } from 'antd';
import { useQuery } from '@tanstack/react-query';
import { desktopApi } from '../services/desktopApi';

const Versioning: React.FC = () => {
  const versionSummaryQuery = useQuery({
    queryKey: ['desktop-version-summary'],
    queryFn: desktopApi.getVersionSummary,
    refetchInterval: 5000,
  });

  if (versionSummaryQuery.isLoading && !versionSummaryQuery.data) {
    return (
      <div style={{ textAlign: 'center', padding: 48 }}>
        <Spin size="large" />
      </div>
    );
  }

  if (versionSummaryQuery.error instanceof Error && !versionSummaryQuery.data) {
    return <Alert type="error" showIcon message="加载版本摘要失败" description={versionSummaryQuery.error.message} />;
  }

  const summary = versionSummaryQuery.data;

  return (
    <Space direction="vertical" size={16} style={{ width: '100%' }}>
      {versionSummaryQuery.error instanceof Error ? (
        <Alert type="error" showIcon message="刷新版本摘要失败" description={versionSummaryQuery.error.message} />
      ) : null}
      {!summary?.available ? (
        <Alert
          type="warning"
          showIcon
          message="版本管理暂不可用"
          description={summary?.error || '当前工作区无法读取 Git 版本摘要。'}
        />
      ) : null}

      <Card title="版本摘要" extra={versionSummaryQuery.isFetching ? '刷新中' : undefined}>
        <Descriptions bordered size="small" column={1}>
          <Descriptions.Item label="可用性">
            <Tag color={summary?.available ? 'success' : 'default'}>{summary?.available ? '可用' : '不可用'}</Tag>
          </Descriptions.Item>
          <Descriptions.Item label="分支">{summary?.branch || '-'}</Descriptions.Item>
          <Descriptions.Item label="工作区状态">
            <Tag color={summary?.dirty ? 'warning' : 'success'}>{summary?.dirty ? '有未提交变更' : '干净'}</Tag>
          </Descriptions.Item>
          <Descriptions.Item label="当前检查点">{summary?.current_checkpoint?.message || '-'}</Descriptions.Item>
          <Descriptions.Item label="错误信息">{summary?.error || '-'}</Descriptions.Item>
        </Descriptions>
      </Card>

      <Card title="工作区变更">
        <List
          locale={{ emptyText: '当前没有未提交变更' }}
          dataSource={summary?.entries || []}
          renderItem={(entry) => (
            <List.Item>
              <Space direction="vertical" size={4} style={{ width: '100%' }}>
                <Typography.Text code>{entry.path}</Typography.Text>
                <Space wrap>
                  <Tag color={statusColor(entry.staged_status)}>staged: {entry.staged_status}</Tag>
                  <Tag color={statusColor(entry.unstaged_status)}>unstaged: {entry.unstaged_status}</Tag>
                </Space>
              </Space>
            </List.Item>
          )}
        />
      </Card>

      <Card title="最近检查点">
        <List
          locale={{ emptyText: '暂无检查点' }}
          dataSource={summary?.checkpoints || []}
          renderItem={(checkpoint) => (
            <List.Item>
              <Space direction="vertical" size={4} style={{ width: '100%' }}>
                <Typography.Text strong>{checkpoint.message}</Typography.Text>
                <Typography.Text code>{checkpoint.revision}</Typography.Text>
                <Typography.Text type="secondary">{formatDateTime(checkpoint.created_at)}</Typography.Text>
              </Space>
            </List.Item>
          )}
        />
      </Card>
    </Space>
  );
};

function statusColor(status: string): string {
  switch (status) {
    case 'added':
      return 'success';
    case 'modified':
      return 'processing';
    case 'deleted':
    case 'unmerged':
      return 'error';
    case 'renamed':
    case 'copied':
      return 'purple';
    case 'untracked':
      return 'gold';
    default:
      return 'default';
  }
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

export default Versioning;

import { Alert, Card, List, Space, Spin, Tag, Typography } from 'antd';
import { useQuery } from '@tanstack/react-query';
import { desktopApi } from '../services/desktopApi';

const levelColor: Record<string, string> = {
  INFO: 'blue',
  WARN: 'orange',
  ERROR: 'red',
};

const Logs: React.FC = () => {
  const logsQuery = useQuery({
    queryKey: ['desktop-logs'],
    queryFn: desktopApi.getLogs,
    refetchInterval: 5000,
  });

  if (logsQuery.isLoading && !logsQuery.data) {
    return (
      <div style={{ textAlign: 'center', padding: 48 }}>
        <Spin size="large" />
      </div>
    );
  }

  if (logsQuery.error instanceof Error && !logsQuery.data) {
    return <Alert type="error" showIcon message="加载日志失败" description={logsQuery.error.message} />;
  }

  return (
    <Space direction="vertical" size={16} style={{ width: '100%' }}>
      {logsQuery.error instanceof Error ? (
        <Alert type="error" showIcon message="刷新日志失败" description={logsQuery.error.message} />
      ) : null}
      <Card title="日志中心" extra={logsQuery.isFetching ? '刷新中' : undefined}>
        <List
          locale={{ emptyText: '暂无日志' }}
          dataSource={logsQuery.data || []}
          renderItem={(entry) => (
            <List.Item>
              <List.Item.Meta
                title={
                  <Space wrap>
                    <Tag color={levelColor[entry.level] || 'default'}>{entry.level}</Tag>
                    <Tag>{entry.source}</Tag>
                    <Typography.Text>{entry.message}</Typography.Text>
                  </Space>
                }
                description={formatDateTime(entry.timestamp)}
              />
            </List.Item>
          )}
        />
      </Card>
    </Space>
  );
};

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

export default Logs;

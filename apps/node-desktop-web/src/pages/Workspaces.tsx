import { Alert, Card, Descriptions, List, Space, Spin, Tag, Typography } from 'antd';
import { useQuery } from '@tanstack/react-query';
import { desktopApi } from '../services/desktopApi';

const Workspaces: React.FC = () => {
  const workspaceStatusQuery = useQuery({
    queryKey: ['desktop-workspace-status'],
    queryFn: desktopApi.getWorkspaceStatus,
    refetchInterval: 5000,
  });

  if (workspaceStatusQuery.isLoading && !workspaceStatusQuery.data) {
    return (
      <div style={{ textAlign: 'center', padding: 48 }}>
        <Spin size="large" />
      </div>
    );
  }

  if (workspaceStatusQuery.error instanceof Error && !workspaceStatusQuery.data) {
    return <Alert type="error" showIcon message="加载工作区失败" description={workspaceStatusQuery.error.message} />;
  }

  const workspace = workspaceStatusQuery.data;

  return (
    <Space direction="vertical" size={16} style={{ width: '100%' }}>
      {workspaceStatusQuery.error instanceof Error ? (
        <Alert type="error" showIcon message="刷新工作区失败" description={workspaceStatusQuery.error.message} />
      ) : null}

      <Card title="工作区管理" extra={workspaceStatusQuery.isFetching ? '刷新中' : undefined}>
        <Descriptions bordered size="small" column={1}>
          <Descriptions.Item label="有效性">
            <Tag color={workspace?.valid ? 'success' : 'error'}>{workspace?.valid ? '有效' : '无效'}</Tag>
          </Descriptions.Item>
          <Descriptions.Item label="名称">{workspace?.name || '-'}</Descriptions.Item>
          <Descriptions.Item label="路径">{workspace?.normalized_path || workspace?.path || '-'}</Descriptions.Item>
          <Descriptions.Item label="Git 仓库">
            <Tag color={workspace?.git_repo ? 'success' : 'default'}>{workspace?.git_repo ? '是' : '否'}</Tag>
          </Descriptions.Item>
          <Descriptions.Item label="只读">
            <Tag color={workspace?.read_only ? 'warning' : 'success'}>{workspace?.read_only ? '是' : '否'}</Tag>
          </Descriptions.Item>
          <Descriptions.Item label="要求 Git 仓库">{workspace?.require_git_repo ? '开启' : '关闭'}</Descriptions.Item>
          <Descriptions.Item label="监听变更">{workspace?.watch_workspace ? '开启' : '关闭'}</Descriptions.Item>
          <Descriptions.Item label="Git 保护">{workspace?.git_protection_enabled ? '开启' : '关闭'}</Descriptions.Item>
          <Descriptions.Item label="自动 git add">{workspace?.auto_git_add_new_files ? '开启' : '关闭'}</Descriptions.Item>
          <Descriptions.Item label="内部目录">{workspace?.internal_work_dir || '-'}</Descriptions.Item>
          <Descriptions.Item label="错误信息">{workspace?.error || '-'}</Descriptions.Item>
        </Descriptions>
      </Card>

      <Card title="路径规则">
        <List
          header={<Typography.Text strong>允许路径模式</Typography.Text>}
          locale={{ emptyText: '未配置允许路径模式' }}
          dataSource={workspace?.allowed_patterns || []}
          renderItem={(pattern) => (
            <List.Item>
              <Typography.Text code>{pattern}</Typography.Text>
            </List.Item>
          )}
        />
        <List
          style={{ marginTop: 16 }}
          header={<Typography.Text strong>拒绝路径模式</Typography.Text>}
          locale={{ emptyText: '未配置拒绝路径模式' }}
          dataSource={workspace?.denied_patterns || []}
          renderItem={(pattern) => (
            <List.Item>
              <Typography.Text code>{pattern}</Typography.Text>
            </List.Item>
          )}
        />
      </Card>
    </Space>
  );
};

export default Workspaces;

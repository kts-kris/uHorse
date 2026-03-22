import { Card, List, Space, Switch, Tag, Typography } from 'antd';
import { workspaces } from '../mock/data';

const Workspaces: React.FC = () => {
  return (
    <Card title="工作区管理">
      <List
        dataSource={workspaces}
        renderItem={(workspace) => (
          <List.Item>
            <Space direction="vertical" size={4} style={{ width: '100%' }}>
              <Space>
                <Typography.Text strong>{workspace.name}</Typography.Text>
                <Tag color={workspace.gitRepo ? 'green' : 'default'}>
                  {workspace.gitRepo ? 'Git' : '非 Git'}
                </Tag>
              </Space>
              <Typography.Text code>{workspace.path}</Typography.Text>
              <Space>
                <Typography.Text type="secondary">只读</Typography.Text>
                <Switch checked={workspace.readOnly} disabled />
              </Space>
            </Space>
          </List.Item>
        )}
      />
    </Card>
  );
};

export default Workspaces;

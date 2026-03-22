import { Alert, Card, List, Space, Tag, Typography } from 'antd';
import { versionEntries } from '../mock/data';

const Versioning: React.FC = () => {
  return (
    <Space direction="vertical" size={16} style={{ width: '100%' }}>
      <Alert
        type="info"
        showIcon
        message="Git-first 版本管理"
        description="当前为桌面端 MVP 骨架，后续将直接接入 uhorse-node-runtime::VersionManager。"
      />
      <Card title="工作区变更">
        <List
          dataSource={versionEntries}
          renderItem={(entry) => (
            <List.Item>
              <Space direction="vertical" size={4} style={{ width: '100%' }}>
                <Typography.Text code>{entry.path}</Typography.Text>
                <Space>
                  <Tag color="blue">staged: {entry.staged}</Tag>
                  <Tag color="gold">unstaged: {entry.unstaged}</Tag>
                </Space>
              </Space>
            </List.Item>
          )}
        />
      </Card>
    </Space>
  );
};

export default Versioning;

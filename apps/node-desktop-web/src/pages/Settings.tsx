import { Card, Descriptions, Tag } from 'antd';

const Settings: React.FC = () => {
  return (
    <Card title="设置">
      <Descriptions bordered size="small" column={1}>
        <Descriptions.Item label="桌面壳">待接入 Tauri 2.x</Descriptions.Item>
        <Descriptions.Item label="自动更新">
          <Tag color="default">未启用</Tag>
        </Descriptions.Item>
        <Descriptions.Item label="通知中心">
          <Tag color="processing">后续接入</Tag>
        </Descriptions.Item>
        <Descriptions.Item label="版本管理后端">
          <Tag color="green">VersionManager</Tag>
        </Descriptions.Item>
      </Descriptions>
    </Card>
  );
};

export default Settings;

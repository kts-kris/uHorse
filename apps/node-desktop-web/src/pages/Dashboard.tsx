import { Card, Col, Descriptions, Row, Space, Statistic, Tag, Typography } from 'antd';
import { ApiOutlined, ClockCircleOutlined, HistoryOutlined, SafetyOutlined } from '@ant-design/icons';
import { overview } from '../mock/data';

const Dashboard: React.FC = () => {
  return (
    <Space direction="vertical" size={16} style={{ width: '100%' }}>
      <Typography.Title level={4} style={{ margin: 0 }}>
        本地 Node 仪表盘
      </Typography.Title>
      <Row gutter={[16, 16]}>
        <Col xs={24} md={6}>
          <Card>
            <Statistic title="连接状态" value={overview.connectionState} prefix={<ApiOutlined />} />
          </Card>
        </Col>
        <Col xs={24} md={6}>
          <Card>
            <Statistic title="运行任务" value={overview.runningTasks} prefix={<ClockCircleOutlined />} />
          </Card>
        </Col>
        <Col xs={24} md={6}>
          <Card>
            <Statistic title="待审批" value={overview.pendingApprovals} prefix={<SafetyOutlined />} />
          </Card>
        </Col>
        <Col xs={24} md={6}>
          <Card>
            <Statistic title="最近检查点" value={overview.latestCheckpoint} prefix={<HistoryOutlined />} />
          </Card>
        </Col>
      </Row>
      <Card title="节点摘要">
        <Descriptions bordered size="small" column={1}>
          <Descriptions.Item label="节点名称">{overview.nodeName}</Descriptions.Item>
          <Descriptions.Item label="工作区路径">{overview.workspacePath}</Descriptions.Item>
          <Descriptions.Item label="Hub 连接">
            <Tag color="green">{overview.connectionState}</Tag>
          </Descriptions.Item>
          <Descriptions.Item label="桌面端阶段">
            <Tag color="processing">MVP Skeleton</Tag>
          </Descriptions.Item>
        </Descriptions>
      </Card>
    </Space>
  );
};

export default Dashboard;

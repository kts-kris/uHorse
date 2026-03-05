import React, { useState } from 'react';
import {
  Card,
  Row,
  Col,
  Statistic,
  Switch,
  Button,
  Tag,
  Space,
  Modal,
  Form,
  Input,
  Select,
  message,
  Descriptions,
  Badge,
  Alert,
  Spin,
} from 'antd';
import {
  ApiOutlined,
  CheckCircleOutlined,
  CloseCircleOutlined,
  ReloadOutlined,
  SettingOutlined,
  TestTubeOutlined,
} from '@ant-design/icons';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';

import { ChannelStatus } from '../types';

// 模拟 API
const channelsApi = {
  listChannels: async (): Promise<ChannelStatus[]> => {
    const response = await fetch('/api/v1/channels');
    if (!response.ok) throw new Error('Failed to fetch channels');
    return response.json();
  },
  getChannelStatus: async (channelType: string): Promise<ChannelStatus> => {
    const response = await fetch(`/api/v1/channels/${channelType}`);
    if (!response.ok) throw new Error('Failed to fetch channel status');
    return response.json();
  },
  enableChannel: async (channelType: string): Promise<void> => {
    const response = await fetch(`/api/v1/channels/${channelType}/enable`, {
      method: 'POST',
    });
    if (!response.ok) throw new Error('Failed to enable channel');
  },
  disableChannel: async (channelType: string): Promise<void> => {
    const response = await fetch(`/api/v1/channels/${channelType}/disable`, {
      method: 'POST',
    });
    if (!response.ok) throw new Error('Failed to disable channel');
  },
  testChannel: async (channelType: string): Promise<{ success: boolean; message: string }> => {
    const response = await fetch(`/api/v1/channels/${channelType}/test`, {
      method: 'POST',
    });
    if (!response.ok) throw new Error('Failed to test channel');
    return response.json();
  },
};

// 通道配置
const CHANNEL_CONFIGS: Record<
  string,
  { name: string; icon: string; description: string }
> = {
  telegram: {
    name: 'Telegram',
    icon: '📱',
    description: 'Telegram Bot API 消息通道',
  },
  dingtalk: {
    name: '钉钉',
    icon: '🔔',
    description: '钉钉企业内部应用机器人',
  },
  feishu: {
    name: '飞书',
    icon: '🚀',
    description: '飞书自建应用消息推送',
  },
  wecom: {
    name: '企业微信',
    icon: '💬',
    description: '企业微信应用消息通道',
  },
  slack: {
    name: 'Slack',
    icon: '💼',
    description: 'Slack App 与 Slash Commands',
  },
  discord: {
    name: 'Discord',
    icon: '🎮',
    description: 'Discord Bot 与 Gateway',
  },
  whatsapp: {
    name: 'WhatsApp',
    icon: '📲',
    description: 'WhatsApp Business API',
  },
};

const Channels: React.FC = () => {
  const queryClient = useQueryClient();
  const [isConfigOpen, setIsConfigOpen] = useState(false);
  const [isTestOpen, setIsTestOpen] = useState(false);
  const [selectedChannel, setSelectedChannel] = useState<string | null>(null);
  const [testResult, setTestResult] = useState<{
    success: boolean;
    message: string;
  } | null>(null);
  const [form] = Form.useForm();

  // 获取通道列表
  const { data: channels, isLoading } = useQuery({
    queryKey: ['channels'],
    queryFn: channelsApi.listChannels,
    refetchInterval: 30000, // 30秒刷新一次
  });

  // 启用通道
  const enableMutation = useMutation({
    mutationFn: channelsApi.enableChannel,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['channels'] });
      message.success('通道已启用');
    },
    onError: () => message.error('启用失败'),
  });

  // 禁用通道
  const disableMutation = useMutation({
    mutationFn: channelsApi.disableChannel,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['channels'] });
      message.success('通道已禁用');
    },
    onError: () => message.error('禁用失败'),
  });

  // 测试通道
  const testMutation = useMutation({
    mutationFn: channelsApi.testChannel,
    onSuccess: (result) => {
      setTestResult(result);
      if (result.success) {
        message.success('通道测试成功');
      } else {
        message.error('通道测试失败');
      }
    },
    onError: () => {
      setTestResult({ success: false, message: '测试请求失败' });
      message.error('测试失败');
    },
  });

  const handleToggle = (channelType: string, enabled: boolean) => {
    if (enabled) {
      enableMutation.mutate(channelType);
    } else {
      disableMutation.mutate(channelType);
    }
  };

  const handleOpenConfig = (channelType: string) => {
    setSelectedChannel(channelType);
    // 加载通道配置
    form.setFieldsValue({
      // 这里应该从 API 获取实际配置
    });
    setIsConfigOpen(true);
  };

  const handleOpenTest = (channelType: string) => {
    setSelectedChannel(channelType);
    setTestResult(null);
    setIsTestOpen(true);
  };

  const handleTest = () => {
    if (selectedChannel) {
      testMutation.mutate(selectedChannel);
    }
  };

  const renderChannelCard = (status: ChannelStatus) => {
    const config = CHANNEL_CONFIGS[status.channel_type] || {
      name: status.channel_type,
      icon: '🔌',
      description: '未知通道类型',
    };

    return (
      <Col key={status.channel_type} xs={24} sm={12} md={8} lg={6}>
        <Card
          hoverable
          actions={[
            <Switch
              key="toggle"
              checked={status.enabled}
              onChange={(checked) => handleToggle(status.channel_type, checked)}
              loading={
                enableMutation.isPending || disableMutation.isPending
              }
            />,
            <Button
              key="config"
              type="text"
              icon={<SettingOutlined />}
              onClick={() => handleOpenConfig(status.channel_type)}
            >
              配置
            </Button>,
            <Button
              key="test"
              type="text"
              icon={<TestTubeOutlined />}
              onClick={() => handleOpenTest(status.channel_type)}
            >
              测试
            </Button>,
          ]}
        >
          <Card.Meta
            avatar={
              <span style={{ fontSize: 32 }}>{config.icon}</span>
            }
            title={
              <Space>
                {config.name}
                {status.running ? (
                  <Badge status="success" />
                ) : (
                  <Badge status="default" />
                )}
              </Space>
            }
            description={config.description}
          />
          <div style={{ marginTop: 16 }}>
            <Space direction="vertical" style={{ width: '100%' }}>
              <div>
                <span style={{ color: '#999' }}>状态: </span>
                {status.connected ? (
                  <Tag color="green" icon={<CheckCircleOutlined />}>
                    已连接
                  </Tag>
                ) : (
                  <Tag color="default" icon={<CloseCircleOutlined />}>
                    未连接
                  </Tag>
                )}
              </div>
              {status.last_activity && (
                <div>
                  <span style={{ color: '#999' }}>最后活动: </span>
                  <span style={{ fontSize: 12 }}>
                    {new Date(status.last_activity).toLocaleString()}
                  </span>
                </div>
              )}
              {status.error && (
                <Alert
                  message={status.error}
                  type="error"
                  showIcon
                  style={{ marginTop: 8 }}
                />
              )}
            </Space>
          </div>
        </Card>
      </Col>
    );
  };

  // 统计数据
  const stats = {
    total: channels?.length || 0,
    enabled: channels?.filter((c) => c.enabled).length || 0,
    connected: channels?.filter((c) => c.connected).length || 0,
  };

  return (
    <div>
      {/* 统计卡片 */}
      <Row gutter={16} style={{ marginBottom: 24 }}>
        <Col xs={24} sm={8}>
          <Card>
            <Statistic
              title="通道总数"
              value={stats.total}
              prefix={<ApiOutlined />}
            />
          </Card>
        </Col>
        <Col xs={24} sm={8}>
          <Card>
            <Statistic
              title="已启用"
              value={stats.enabled}
              valueStyle={{ color: '#3f8600' }}
              prefix={<CheckCircleOutlined />}
            />
          </Card>
        </Col>
        <Col xs={24} sm={8}>
          <Card>
            <Statistic
              title="已连接"
              value={stats.connected}
              valueStyle={{ color: '#1890ff' }}
              prefix={<ApiOutlined />}
            />
          </Card>
        </Col>
      </Row>

      {/* 通道列表 */}
      <Card
        title="通道管理"
        extra={
          <Button
            icon={<ReloadOutlined />}
            onClick={() => queryClient.invalidateQueries({ queryKey: ['channels'] })}
          >
            刷新
          </Button>
        }
      >
        {isLoading ? (
          <div style={{ textAlign: 'center', padding: 40 }}>
            <Spin size="large" />
          </div>
        ) : (
          <Row gutter={[16, 16]}>
            {channels?.map(renderChannelCard)}
          </Row>
        )}
      </Card>

      {/* 配置 Modal */}
      <Modal
        title={`${CHANNEL_CONFIGS[selectedChannel || '']?.name || selectedChannel} 配置`}
        open={isConfigOpen}
        onCancel={() => setIsConfigOpen(false)}
        onOk={() => form.submit()}
        width={600}
      >
        <Form form={form} layout="vertical">
          {selectedChannel === 'telegram' && (
            <>
              <Form.Item
                name="bot_token"
                label="Bot Token"
                rules={[{ required: true }]}
              >
                <Input.Password placeholder="123456789:ABCdefGHIjklMNOpqrsTUVwxyz" />
              </Form.Item>
              <Form.Item name="webhook_url" label="Webhook URL">
                <Input placeholder="https://your-domain.com/api/webhooks/telegram" />
              </Form.Item>
            </>
          )}
          {selectedChannel === 'dingtalk' && (
            <>
              <Form.Item
                name="app_key"
                label="AppKey"
                rules={[{ required: true }]}
              >
                <Input placeholder="钉钉应用的 AppKey" />
              </Form.Item>
              <Form.Item
                name="app_secret"
                label="AppSecret"
                rules={[{ required: true }]}
              >
                <Input.Password placeholder="钉钉应用的 AppSecret" />
              </Form.Item>
            </>
          )}
          {selectedChannel === 'feishu' && (
            <>
              <Form.Item
                name="app_id"
                label="App ID"
                rules={[{ required: true }]}
              >
                <Input placeholder="飞书应用的 App ID" />
              </Form.Item>
              <Form.Item
                name="app_secret"
                label="App Secret"
                rules={[{ required: true }]}
              >
                <Input.Password placeholder="飞书应用的 App Secret" />
              </Form.Item>
            </>
          )}
        </Form>
      </Modal>

      {/* 测试 Modal */}
      <Modal
        title={`测试 ${CHANNEL_CONFIGS[selectedChannel || '']?.name || selectedChannel} 通道`}
        open={isTestOpen}
        onCancel={() => setIsTestOpen(false)}
        footer={[
          <Button key="cancel" onClick={() => setIsTestOpen(false)}>
            关闭
          </Button>,
          <Button
            key="test"
            type="primary"
            loading={testMutation.isPending}
            onClick={handleTest}
          >
            发送测试消息
          </Button>,
        ]}
      >
        {testResult && (
          <Alert
            message={testResult.success ? '测试成功' : '测试失败'}
            description={testResult.message}
            type={testResult.success ? 'success' : 'error'}
            showIcon
            style={{ marginBottom: 16 }}
          />
        )}
        <p>点击下方按钮发送测试消息到配置的通道。</p>
      </Modal>
    </div>
  );
};

export default Channels;

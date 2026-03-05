import React, { useState } from 'react';
import {
  Card,
  Tabs,
  Form,
  Input,
  Button,
  Switch,
  InputNumber,
  Select,
  Divider,
  message,
  Space,
  Table,
  Tag,
  Descriptions,
  Statistic,
  Row,
  Col,
  Progress,
  Alert,
} from 'antd';
import {
  SaveOutlined,
  ReloadOutlined,
  DatabaseOutlined,
  SettingOutlined,
  SafetyOutlined,
  DashboardOutlined,
} from '@ant-design/icons';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';

import { SystemInfo, SystemMetrics } from '../types';

// 模拟 API
const systemApi = {
  getSystemInfo: async (): Promise<SystemInfo> => {
    const response = await fetch('/api/v1/system/info');
    if (!response.ok) throw new Error('Failed to fetch system info');
    return response.json();
  },
  getMetrics: async (): Promise<SystemMetrics> => {
    const response = await fetch('/api/v1/system/metrics');
    if (!response.ok) throw new Error('Failed to fetch metrics');
    return response.json();
  },
  updateConfig: async (config: Record<string, unknown>): Promise<void> => {
    const response = await fetch('/api/v1/system/config', {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(config),
    });
    if (!response.ok) throw new Error('Failed to update config');
  },
};

const Settings: React.FC = () => {
  const queryClient = useQueryClient();
  const [activeTab, setActiveTab] = useState('general');
  const [generalForm] = Form.useForm();
  const [llmForm] = Form.useForm();
  const [securityForm] = Form.useForm();

  // 获取系统信息
  const { data: systemInfo } = useQuery({
    queryKey: ['systemInfo'],
    queryFn: systemApi.getSystemInfo,
    refetchInterval: 60000, // 1分钟刷新
  });

  // 获取系统指标
  const { data: metrics } = useQuery({
    queryKey: ['metrics'],
    queryFn: systemApi.getMetrics,
    refetchInterval: 10000, // 10秒刷新
  });

  // 更新配置
  const updateConfigMutation = useMutation({
    mutationFn: systemApi.updateConfig,
    onSuccess: () => {
      message.success('配置已保存');
      queryClient.invalidateQueries({ queryKey: ['systemInfo'] });
    },
    onError: () => message.error('保存失败'),
  });

  const handleSaveGeneral = async () => {
    const values = await generalForm.validateFields();
    updateConfigMutation.mutate(values);
  };

  const handleSaveLlm = async () => {
    const values = await llmForm.validateFields();
    updateConfigMutation.mutate({ llm: values });
  };

  const handleSaveSecurity = async () => {
    const values = await securityForm.validateFields();
    updateConfigMutation.mutate({ security: values });
  };

  // 系统概览 Tab
  const OverviewTab = () => (
    <div>
      <Row gutter={16}>
        <Col span={8}>
          <Card>
            <Statistic
              title="运行时间"
              value={systemInfo?.uptime_secs || 0}
              suffix="秒"
              prefix={<DashboardOutlined />}
            />
          </Card>
        </Col>
        <Col span={8}>
          <Card>
            <Statistic
              title="今日消息"
              value={metrics?.messages_today || 0}
              prefix={<DatabaseOutlined />}
            />
          </Card>
        </Col>
        <Col span={8}>
          <Card>
            <Statistic
              title="平均响应时间"
              value={metrics?.avg_response_time_ms || 0}
              suffix="ms"
            />
          </Card>
        </Col>
      </Row>

      <Divider />

      <Descriptions title="系统信息" bordered column={2}>
        <Descriptions.Item label="名称">{systemInfo?.name}</Descriptions.Item>
        <Descriptions.Item label="版本">{systemInfo?.version}</Descriptions.Item>
        <Descriptions.Item label="Rust 版本">
          {systemInfo?.rust_version}
        </Descriptions.Item>
        <Descriptions.Item label="通道数量">
          {systemInfo?.channels_count}
        </Descriptions.Item>
        <Descriptions.Item label="Agent 数量">
          {systemInfo?.agents_count}
        </Descriptions.Item>
        <Descriptions.Item label="活跃会话">
          {systemInfo?.active_sessions}
        </Descriptions.Item>
      </Descriptions>

      <Divider />

      <Descriptions title="运行指标" bordered column={2}>
        <Descriptions.Item label="总消息数">
          {metrics?.total_messages}
        </Descriptions.Item>
        <Descriptions.Item label="今日消息">
          {metrics?.messages_today}
        </Descriptions.Item>
        <Descriptions.Item label="总请求数">
          {metrics?.total_requests}
        </Descriptions.Item>
        <Descriptions.Item label="错误数">
          <Tag color={metrics?.total_errors ? 'red' : 'green'}>
            {metrics?.total_errors || 0}
          </Tag>
        </Descriptions.Item>
        <Descriptions.Item label="内存使用">
          {((metrics?.memory_usage_bytes || 0) / 1024 / 1024).toFixed(2)} MB
        </Descriptions.Item>
      </Descriptions>
    </div>
  );

  // 通用设置 Tab
  const GeneralTab = () => (
    <Card title="通用设置">
      <Form form={generalForm} layout="vertical">
        <Form.Item name="server_host" label="服务地址" initialValue="0.0.0.0">
          <Input placeholder="0.0.0.0" />
        </Form.Item>
        <Form.Item name="server_port" label="服务端口" initialValue={8080}>
          <InputNumber min={1} max={65535} style={{ width: '100%' }} />
        </Form.Item>
        <Form.Item
          name="log_level"
          label="日志级别"
          initialValue="info"
        >
          <Select
            options={[
              { label: 'Debug', value: 'debug' },
              { label: 'Info', value: 'info' },
              { label: 'Warn', value: 'warn' },
              { label: 'Error', value: 'error' },
            ]}
          />
        </Form.Item>
        <Form.Item
          name="max_connections"
          label="最大连接数"
          initialValue={1000}
        >
          <InputNumber min={1} max={100000} style={{ width: '100%' }} />
        </Form.Item>
        <Form.Item>
          <Button
            type="primary"
            icon={<SaveOutlined />}
            onClick={handleSaveGeneral}
            loading={updateConfigMutation.isPending}
          >
            保存设置
          </Button>
        </Form.Item>
      </Form>
    </Card>
  );

  // LLM 设置 Tab
  const LlmTab = () => (
    <Card title="LLM 设置">
      <Form form={llmForm} layout="vertical">
        <Divider>默认模型</Divider>
        <Form.Item
          name="default_model"
          label="默认模型"
          initialValue="gpt-4"
        >
          <Select
            options={[
              { label: 'GPT-4', value: 'gpt-4' },
              { label: 'GPT-4 Turbo', value: 'gpt-4-turbo' },
              { label: 'GPT-3.5 Turbo', value: 'gpt-3.5-turbo' },
              { label: 'Claude 3 Opus', value: 'claude-3-opus' },
              { label: 'Claude 3 Sonnet', value: 'claude-3-sonnet' },
            ]}
          />
        </Form.Item>
        <Form.Item name="api_key" label="OpenAI API Key">
          <Input.Password placeholder="sk-..." />
        </Form.Item>
        <Form.Item name="api_base" label="API Base URL">
          <Input placeholder="https://api.openai.com/v1" />
        </Form.Item>

        <Divider>模型参数</Divider>
        <Form.Item name="default_temperature" label="默认温度" initialValue={0.7}>
          <InputNumber min={0} max={2} step={0.1} style={{ width: '100%' }} />
        </Form.Item>
        <Form.Item name="default_max_tokens" label="默认最大 Token" initialValue={4096}>
          <InputNumber min={100} max={128000} step={100} style={{ width: '100%' }} />
        </Form.Item>
        <Form.Item name="timeout_secs" label="请求超时 (秒)" initialValue={60}>
          <InputNumber min={10} max={600} style={{ width: '100%' }} />
        </Form.Item>

        <Form.Item>
          <Button
            type="primary"
            icon={<SaveOutlined />}
            onClick={handleSaveLlm}
            loading={updateConfigMutation.isPending}
          >
            保存设置
          </Button>
        </Form.Item>
      </Form>
    </Card>
  );

  // 安全设置 Tab
  const SecurityTab = () => (
    <Card title="安全设置">
      <Alert
        message="注意"
        description="修改安全设置后可能需要重启服务才能生效。"
        type="warning"
        showIcon
        style={{ marginBottom: 16 }}
      />
      <Form form={securityForm} layout="vertical">
        <Form.Item
          name="jwt_enabled"
          label="启用 JWT 认证"
          valuePropName="checked"
          initialValue={true}
        >
          <Switch />
        </Form.Item>
        <Form.Item name="jwt_secret" label="JWT Secret">
          <Input.Password placeholder="用于签名 JWT Token 的密钥" />
        </Form.Item>
        <Form.Item
          name="token_expire_secs"
          label="Token 过期时间 (秒)"
          initialValue={3600}
        >
          <InputNumber min={60} max={86400} style={{ width: '100%' }} />
        </Form.Item>

        <Divider>访问控制</Divider>
        <Form.Item
          name="rate_limit_enabled"
          label="启用速率限制"
          valuePropName="checked"
          initialValue={true}
        >
          <Switch />
        </Form.Item>
        <Form.Item
          name="rate_limit_per_minute"
          label="每分钟请求限制"
          initialValue={60}
        >
          <InputNumber min={1} max={10000} style={{ width: '100%' }} />
        </Form.Item>
        <Form.Item name="allowed_origins" label="允许的 CORS 来源">
          <Select
            mode="tags"
            placeholder="例如: https://example.com"
            style={{ width: '100%' }}
          />
        </Form.Item>

        <Form.Item>
          <Button
            type="primary"
            icon={<SafetyOutlined />}
            onClick={handleSaveSecurity}
            loading={updateConfigMutation.isPending}
          >
            保存设置
          </Button>
        </Form.Item>
      </Form>
    </Card>
  );

  return (
    <Card
      title="系统设置"
      extra={
        <Button
          icon={<ReloadOutlined />}
          onClick={() => {
            queryClient.invalidateQueries({ queryKey: ['systemInfo'] });
            queryClient.invalidateQueries({ queryKey: ['metrics'] });
          }}
        >
          刷新
        </Button>
      }
    >
      <Tabs
        activeKey={activeTab}
        onChange={setActiveTab}
        items={[
          {
            key: 'overview',
            label: (
              <span>
                <DashboardOutlined />
                系统概览
              </span>
            ),
            children: <OverviewTab />,
          },
          {
            key: 'general',
            label: (
              <span>
                <SettingOutlined />
                通用设置
              </span>
            ),
            children: <GeneralTab />,
          },
          {
            key: 'llm',
            label: (
              <span>
                <DatabaseOutlined />
                LLM 设置
              </span>
            ),
            children: <LlmTab />,
          },
          {
            key: 'security',
            label: (
              <span>
                <SafetyOutlined />
                安全设置
              </span>
            ),
            children: <SecurityTab />,
          },
        ]}
      />
    </Card>
  );
};

export default Settings;

import React, { useMemo } from 'react';
import { useQuery } from '@tanstack/react-query';
import {
  Alert,
  Card,
  Col,
  Descriptions,
  Empty,
  List,
  Row,
  Space,
  Spin,
  Statistic,
  Tag,
  Typography,
} from 'antd';
import {
  ApiOutlined,
  CheckCircleOutlined,
  ClockCircleOutlined,
  DisconnectOutlined,
} from '@ant-design/icons';

import { systemService } from '../services/system';

const Channels: React.FC = () => {
  const {
    data: stats,
    isLoading: statsLoading,
    error: statsError,
  } = useQuery({
    queryKey: ['hub-stats'],
    queryFn: systemService.getStats,
    refetchInterval: 5000,
  });

  const {
    data: channels = [],
    isLoading: channelsLoading,
    error: channelsError,
  } = useQuery({
    queryKey: ['runtime-channels'],
    queryFn: systemService.getChannels,
    refetchInterval: 5000,
  });

  const {
    data: nodes = [],
    isLoading: nodesLoading,
    error: nodesError,
  } = useQuery({
    queryKey: ['runtime-nodes'],
    queryFn: systemService.getNodes,
    refetchInterval: 5000,
  });

  const loading = statsLoading || channelsLoading || nodesLoading;

  const channelStats = useMemo(() => {
    const onlineNodes = nodes.filter((node) => node.state.toLowerCase() === 'online').length;
    const busyNodes = nodes.filter((node) => node.state.toLowerCase() === 'busy').length;
    const taskNodes = nodes.filter((node) => node.current_tasks > 0).length;

    return {
      onlineNodes,
      busyNodes,
      taskNodes,
      totalNodes: stats?.nodes.total_nodes || nodes.length,
      registeredChannels: channels.filter((channel) => channel.registered).length,
    };
  }, [channels, nodes, stats]);

  if (loading && !stats) {
    return (
      <div style={{ textAlign: 'center', padding: 48 }}>
        <Spin size="large" />
      </div>
    );
  }

  return (
    <Space direction="vertical" size={16} style={{ width: '100%' }}>
      <Alert
        type="info"
        showIcon
        message="当前控制面暂无独立的通道启停 / 测试 API。"
        description="此页展示 Hub 当前进程已注册 channel 的运行时能力与节点侧状态；当前 Phase0 仅收口 Telegram / DingTalk / Slack / Discord / WhatsApp，完整交互主线仍以 DingTalk 为主。"
      />

      {statsError && (
        <Alert
          type="error"
          showIcon
          message="加载 Hub 统计失败"
          description={statsError instanceof Error ? statsError.message : '未知错误'}
        />
      )}

      {channelsError && (
        <Alert
          type="error"
          showIcon
          message="加载通道能力失败"
          description={channelsError instanceof Error ? channelsError.message : '未知错误'}
        />
      )}

      {nodesError && (
        <Alert
          type="error"
          showIcon
          message="加载节点状态失败"
          description={nodesError instanceof Error ? nodesError.message : '未知错误'}
        />
      )}

      <Row gutter={[16, 16]}>
        <Col xs={24} sm={12} md={6}>
          <Card>
            <Statistic
              title="接入入口"
              value={channelStats.registeredChannels}
              prefix={<ApiOutlined />}
              suffix="个"
            />
          </Card>
        </Col>
        <Col xs={24} sm={12} md={6}>
          <Card>
            <Statistic title="在线节点" value={channelStats.onlineNodes} prefix={<CheckCircleOutlined />} />
          </Card>
        </Col>
        <Col xs={24} sm={12} md={6}>
          <Card>
            <Statistic title="忙碌节点" value={channelStats.busyNodes} prefix={<ClockCircleOutlined />} />
          </Card>
        </Col>
        <Col xs={24} sm={12} md={6}>
          <Card>
            <Statistic title="承载任务节点" value={channelStats.taskNodes} prefix={<DisconnectOutlined />} />
          </Card>
        </Col>
      </Row>

      <Card title="已注册消息入口与运行时能力">
        <List
          dataSource={channels}
          locale={{ emptyText: '暂无 channel 能力数据' }}
          renderItem={(item) => (
            <List.Item key={item.channel_type}>
              <Descriptions size="small" column={1} style={{ width: '100%' }}>
                <Descriptions.Item label="名称">
                  <Space>
                    <Typography.Text strong>{item.channel_type}</Typography.Text>
                    <Tag color={item.registered ? 'green' : 'default'}>
                      {item.registered ? '已注册' : '未注册'}
                    </Tag>
                  </Space>
                </Descriptions.Item>
                <Descriptions.Item label="能力">
                  {item.capabilities.length === 0 ? (
                    <Typography.Text type="secondary">当前未注册或未声明运行时能力</Typography.Text>
                  ) : (
                    <Space size={[4, 4]} wrap>
                      {item.capabilities.map((capability) => (
                        <Tag key={capability}>{formatChannelCapability(capability)}</Tag>
                      ))}
                    </Space>
                  )}
                </Descriptions.Item>
                <Descriptions.Item label="说明">
                  {describeChannelEntry(item.channel_type, item.capabilities)}
                </Descriptions.Item>
              </Descriptions>
            </List.Item>
          )}
        />
      </Card>

      <Card title="节点侧承载状态">
        {nodes.length === 0 ? (
          <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description="暂无已注册节点" />
        ) : (
          <List
            dataSource={nodes}
            renderItem={(node) => (
              <List.Item key={node.node_id}>
                <Row gutter={[16, 16]} style={{ width: '100%' }}>
                  <Col xs={24} md={8}>
                    <Space direction="vertical" size={4}>
                      <Space>
                        <Typography.Text strong>{node.name}</Typography.Text>
                        <Tag color={nodeStateColor(node.state)}>{node.state}</Tag>
                      </Space>
                      <Typography.Text code>{node.node_id}</Typography.Text>
                      <Typography.Text type="secondary">{node.workspace.path}</Typography.Text>
                    </Space>
                  </Col>
                  <Col xs={24} md={8}>
                    <Descriptions size="small" column={1}>
                      <Descriptions.Item label="当前任务">{node.current_tasks}</Descriptions.Item>
                      <Descriptions.Item label="已完成">{node.completed_tasks}</Descriptions.Item>
                      <Descriptions.Item label="失败">{node.failed_tasks}</Descriptions.Item>
                    </Descriptions>
                  </Col>
                  <Col xs={24} md={8}>
                    <Descriptions size="small" column={1}>
                      <Descriptions.Item label="支持命令">
                        <Space size={[4, 4]} wrap>
                          {node.capabilities.supported_commands.map((command) => (
                            <Tag key={command}>{command}</Tag>
                          ))}
                        </Space>
                      </Descriptions.Item>
                      <Descriptions.Item label="最后心跳">
                        {formatDateTime(node.last_heartbeat)}
                      </Descriptions.Item>
                    </Descriptions>
                  </Col>
                </Row>
              </List.Item>
            )}
          />
        )}
      </Card>
    </Space>
  );
};

function formatChannelCapability(capability: string): string {
  switch (capability) {
    case 'send_to_recipient':
      return '通用发送';
    case 'inbound_webhook':
      return '入站 Webhook';
    case 'reply_context':
      return '上下文回复';
    default:
      return capability;
  }
}

function describeChannelEntry(channelType: string, capabilities: string[]): string {
  if (capabilities.includes('reply_context')) {
    return `${channelType} 已声明上下文回复能力，可承接当前主线中的回复路由语义。`;
  }
  if (capabilities.includes('inbound_webhook')) {
    return `${channelType} 已声明入站 Webhook 能力，可承接平台回调入口。`;
  }
  if (capabilities.includes('send_to_recipient')) {
    return `${channelType} 当前仅声明通用收件人发送能力，未承诺完整回复主链。`;
  }
  return `${channelType} 当前不在本进程注册表中，或不属于本阶段已接线入口。`;
}

function formatDateTime(value: string): string {
  const timestamp = Date.parse(value);
  if (Number.isNaN(timestamp)) {
    return value;
  }
  return new Date(timestamp).toLocaleString();
}

function nodeStateColor(state: string): string {
  switch (state.toLowerCase()) {
    case 'online':
      return 'green';
    case 'busy':
      return 'orange';
    case 'offline':
      return 'default';
    case 'maintenance':
      return 'purple';
    default:
      return 'blue';
  }
}

export default Channels;

import React, { useState } from 'react';
import {
  Table,
  Button,
  Space,
  Modal,
  Tag,
  message,
  Popconfirm,
  Card,
  Descriptions,
  Timeline,
  Input,
  Select,
  Badge,
  Drawer,
  List,
} from 'antd';
import {
  EyeOutlined,
  DeleteOutlined,
  MessageOutlined,
  CloseCircleOutlined,
  SearchOutlined,
} from '@ant-design/icons';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import type { ColumnsType } from 'antd/es/table';
import dayjs from 'dayjs';

import { Session, SessionMessage, SessionStatus } from '../types';

// 模拟 API
const sessionsApi = {
  listSessions: async (params?: {
    agent_id?: string;
    status?: SessionStatus;
  }): Promise<Session[]> => {
    const query = new URLSearchParams();
    if (params?.agent_id) query.set('agent_id', params.agent_id);
    if (params?.status) query.set('status', params.status);
    const response = await fetch(`/api/v1/sessions?${query}`);
    if (!response.ok) throw new Error('Failed to fetch sessions');
    return response.json();
  },
  getSession: async (id: string): Promise<Session> => {
    const response = await fetch(`/api/v1/sessions/${id}`);
    if (!response.ok) throw new Error('Failed to fetch session');
    return response.json();
  },
  getSessionMessages: async (id: string): Promise<SessionMessage[]> => {
    const response = await fetch(`/api/v1/sessions/${id}/messages`);
    if (!response.ok) throw new Error('Failed to fetch messages');
    return response.json();
  },
  deleteSession: async (id: string): Promise<void> => {
    const response = await fetch(`/api/v1/sessions/${id}`, {
      method: 'DELETE',
    });
    if (!response.ok) throw new Error('Failed to delete session');
  },
};

const Sessions: React.FC = () => {
  const queryClient = useQueryClient();
  const [isDetailOpen, setIsDetailOpen] = useState(false);
  const [isMessagesOpen, setIsMessagesOpen] = useState(false);
  const [selectedSession, setSelectedSession] = useState<Session | null>(null);
  const [searchParams, setSearchParams] = useState<{
    agent_id?: string;
    status?: SessionStatus;
  }>({});

  // 获取会话列表
  const { data: sessions, isLoading } = useQuery({
    queryKey: ['sessions', searchParams],
    queryFn: () => sessionsApi.listSessions(searchParams),
  });

  // 获取消息历史
  const { data: messages, isLoading: isLoadingMessages } = useQuery({
    queryKey: ['session-messages', selectedSession?.id],
    queryFn: () =>
      selectedSession ? sessionsApi.getSessionMessages(selectedSession.id) : [],
    enabled: !!selectedSession && isMessagesOpen,
  });

  // 删除会话
  const deleteMutation = useMutation({
    mutationFn: sessionsApi.deleteSession,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['sessions'] });
      message.success('会话删除成功');
    },
    onError: () => message.error('会话删除失败'),
  });

  const handleViewDetail = async (session: Session) => {
    setSelectedSession(session);
    setIsDetailOpen(true);
  };

  const handleViewMessages = (session: Session) => {
    setSelectedSession(session);
    setIsMessagesOpen(true);
  };

  const renderStatus = (status: SessionStatus) => {
    const config: Record<
      SessionStatus,
      { color: string; text: string; status: 'success' | 'processing' | 'default' | 'error' }
    > = {
      active: { color: 'green', text: '活跃', status: 'success' },
      paused: { color: 'orange', text: '暂停', status: 'processing' },
      closed: { color: 'default', text: '已关闭', status: 'default' },
    };
    const { color, text, status } = config[status];
    return <Badge status={status} text={<Tag color={color}>{text}</Tag>} />;
  };

  const renderChannelType = (type: string) => {
    const colors: Record<string, string> = {
      telegram: 'blue',
      dingtalk: 'cyan',
      feishu: 'geekblue',
      wecom: 'green',
      slack: 'purple',
      discord: 'magenta',
      whatsapp: 'lime',
    };
    return <Tag color={colors[type] || 'default'}>{type.toUpperCase()}</Tag>;
  };

  const columns: ColumnsType<Session> = [
    {
      title: '会话 ID',
      dataIndex: 'id',
      key: 'id',
      width: 280,
      render: (id) => (
        <code style={{ fontSize: 12, color: '#666' }}>
          {id.substring(0, 8)}...
        </code>
      ),
    },
    {
      title: 'Agent',
      dataIndex: 'agent_id',
      key: 'agent_id',
      width: 200,
      render: (agentId) => (
        <Tag color="blue">{agentId.substring(0, 12)}...</Tag>
      ),
    },
    {
      title: '通道',
      dataIndex: 'channel_type',
      key: 'channel_type',
      width: 120,
      render: renderChannelType,
    },
    {
      title: '状态',
      dataIndex: 'status',
      key: 'status',
      width: 100,
      render: renderStatus,
    },
    {
      title: '创建时间',
      dataIndex: 'created_at',
      key: 'created_at',
      width: 180,
      render: (time) => dayjs(time).format('YYYY-MM-DD HH:mm:ss'),
    },
    {
      title: '更新时间',
      dataIndex: 'updated_at',
      key: 'updated_at',
      width: 180,
      render: (time) => dayjs(time).format('YYYY-MM-DD HH:mm:ss'),
    },
    {
      title: '操作',
      key: 'actions',
      width: 200,
      render: (_, record) => (
        <Space>
          <Button
            type="link"
            icon={<EyeOutlined />}
            onClick={() => handleViewDetail(record)}
          >
            详情
          </Button>
          <Button
            type="link"
            icon={<MessageOutlined />}
            onClick={() => handleViewMessages(record)}
          >
            消息
          </Button>
          <Popconfirm
            title="确定要删除此会话吗？"
            onConfirm={() => deleteMutation.mutate(record.id)}
          >
            <Button type="link" danger icon={<DeleteOutlined />}>
              删除
            </Button>
          </Popconfirm>
        </Space>
      ),
    },
  ];

  return (
    <Card title="会话管理">
      {/* 搜索过滤 */}
      <Space style={{ marginBottom: 16 }}>
        <Input
          placeholder="Agent ID"
          value={searchParams.agent_id}
          onChange={(e) =>
            setSearchParams({ ...searchParams, agent_id: e.target.value })
          }
          style={{ width: 200 }}
          prefix={<SearchOutlined />}
          allowClear
        />
        <Select
          placeholder="状态筛选"
          value={searchParams.status}
          onChange={(status) => setSearchParams({ ...searchParams, status })}
          style={{ width: 120 }}
          allowClear
          options={[
            { label: '活跃', value: 'active' },
            { label: '暂停', value: 'paused' },
            { label: '已关闭', value: 'closed' },
          ]}
        />
      </Space>

      <Table
        columns={columns}
        dataSource={sessions}
        rowKey="id"
        loading={isLoading}
        pagination={{
          showSizeChanger: true,
          showQuickJumper: true,
          showTotal: (total) => `共 ${total} 条`,
        }}
      />

      {/* 会话详情 Drawer */}
      <Drawer
        title="会话详情"
        open={isDetailOpen}
        onClose={() => setIsDetailOpen(false)}
        width={500}
      >
        {selectedSession && (
          <Descriptions column={1} bordered>
            <Descriptions.Item label="会话 ID">
              <code>{selectedSession.id}</code>
            </Descriptions.Item>
            <Descriptions.Item label="Agent ID">
              <code>{selectedSession.agent_id}</code>
            </Descriptions.Item>
            <Descriptions.Item label="通道类型">
              {renderChannelType(selectedSession.channel_type)}
            </Descriptions.Item>
            <Descriptions.Item label="状态">
              {renderStatus(selectedSession.status)}
            </Descriptions.Item>
            <Descriptions.Item label="创建时间">
              {dayjs(selectedSession.created_at).format('YYYY-MM-DD HH:mm:ss')}
            </Descriptions.Item>
            <Descriptions.Item label="更新时间">
              {dayjs(selectedSession.updated_at).format('YYYY-MM-DD HH:mm:ss')}
            </Descriptions.Item>
          </Descriptions>
        )}
      </Drawer>

      {/* 消息历史 Drawer */}
      <Drawer
        title="消息历史"
        open={isMessagesOpen}
        onClose={() => setIsMessagesOpen(false)}
        width={600}
      >
        {isLoadingMessages ? (
          <div style={{ textAlign: 'center', padding: 40 }}>加载中...</div>
        ) : (
          <List
            dataSource={messages}
            renderItem={(msg) => (
              <List.Item>
                <div style={{ width: '100%' }}>
                  <Space style={{ marginBottom: 8 }}>
                    <Tag
                      color={
                        msg.role === 'user'
                          ? 'blue'
                          : msg.role === 'assistant'
                          ? 'green'
                          : msg.role === 'system'
                          ? 'purple'
                          : 'orange'
                      }
                    >
                      {msg.role}
                    </Tag>
                    <span style={{ color: '#999', fontSize: 12 }}>
                      {dayjs(msg.created_at).format('HH:mm:ss')}
                    </span>
                  </Space>
                  <div
                    style={{
                      padding: '8px 12px',
                      background: '#f5f5f5',
                      borderRadius: 8,
                      whiteSpace: 'pre-wrap',
                    }}
                  >
                    {msg.content}
                  </div>
                </div>
              </List.Item>
            )}
          />
        )}
      </Drawer>
    </Card>
  );
};

export default Sessions;

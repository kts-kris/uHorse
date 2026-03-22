import React, { useMemo, useState } from 'react';
import {
  Alert,
  Button,
  Card,
  Col,
  Descriptions,
  Drawer,
  Empty,
  Input,
  List,
  Row,
  Select,
  Space,
  Statistic,
  Table,
  Tag,
  Typography,
} from 'antd';
import {
  EyeOutlined,
  MessageOutlined,
  ReloadOutlined,
  RobotOutlined,
  SearchOutlined,
} from '@ant-design/icons';
import { useQuery } from '@tanstack/react-query';
import type { ColumnsType } from 'antd/es/table';

import type { SessionRuntimeSummary } from '../types';
import { sessionService } from '../services/agents';

const Sessions: React.FC = () => {
  const [selectedSessionId, setSelectedSessionId] = useState<string | null>(null);
  const [keyword, setKeyword] = useState('');
  const [agentFilter, setAgentFilter] = useState<string | undefined>(undefined);

  const {
    data: sessions = [],
    isLoading,
    isFetching,
    error,
    refetch,
  } = useQuery({
    queryKey: ['sessions-runtime'],
    queryFn: sessionService.list,
  });

  const {
    data: sessionDetail,
    isLoading: isDetailLoading,
    error: detailError,
  } = useQuery({
    queryKey: ['session-runtime', selectedSessionId],
    queryFn: () => sessionService.get(selectedSessionId!),
    enabled: selectedSessionId !== null,
  });

  const {
    data: messages = [],
    isLoading: isMessagesLoading,
    error: messagesError,
  } = useQuery({
    queryKey: ['session-runtime-messages', selectedSessionId],
    queryFn: () => sessionService.getMessages(selectedSessionId!),
    enabled: selectedSessionId !== null,
  });

  const filteredSessions = useMemo(() => {
    const normalizedKeyword = keyword.trim().toLowerCase();
    return [...sessions]
      .filter((session) => {
        if (agentFilter && session.agent_id !== agentFilter) {
          return false;
        }

        if (!normalizedKeyword) {
          return true;
        }

        return [
          session.session_id,
          session.agent_id,
          session.conversation_id,
          session.sender_user_id,
          session.sender_staff_id,
        ]
          .filter(Boolean)
          .some((value) => value!.toLowerCase().includes(normalizedKeyword));
      })
      .sort(
        (left, right) =>
          new Date(right.last_active).getTime() - new Date(left.last_active).getTime()
      );
  }, [agentFilter, keyword, sessions]);

  const agentOptions = useMemo(
    () =>
      Array.from(new Set(sessions.map((session) => session.agent_id).filter(Boolean))).map(
        (agentId) => ({ label: agentId!, value: agentId! })
      ),
    [sessions]
  );

  const stats = useMemo(() => {
    return {
      totalSessions: sessions.length,
      filteredSessions: filteredSessions.length,
      totalMessages: filteredSessions.reduce((sum, session) => sum + session.message_count, 0),
      boundAgents: new Set(filteredSessions.map((session) => session.agent_id).filter(Boolean)).size,
    };
  }, [filteredSessions, sessions]);

  const columns: ColumnsType<SessionRuntimeSummary> = [
    {
      title: 'Session',
      dataIndex: 'session_id',
      key: 'session_id',
      width: 260,
      render: (value: string) => (
        <Typography.Text code ellipsis={{ tooltip: value }}>
          {value}
        </Typography.Text>
      ),
    },
    {
      title: 'Agent',
      dataIndex: 'agent_id',
      key: 'agent_id',
      width: 160,
      render: (value: string | null) => (value ? <Tag color="blue">{value}</Tag> : <Tag>未绑定</Tag>),
    },
    {
      title: 'Conversation',
      dataIndex: 'conversation_id',
      key: 'conversation_id',
      ellipsis: true,
      render: (value: string | null) => value || '-',
    },
    {
      title: '消息数',
      dataIndex: 'message_count',
      key: 'message_count',
      width: 90,
    },
    {
      title: '最近活跃',
      dataIndex: 'last_active',
      key: 'last_active',
      width: 200,
      render: (value: string) => formatDateTime(value),
    },
    {
      title: '操作',
      key: 'actions',
      width: 100,
      render: (_, record) => (
        <Button
          type="link"
          icon={<EyeOutlined />}
          onClick={() => setSelectedSessionId(record.session_id)}
        >
          详情
        </Button>
      ),
    },
  ];

  return (
    <div>
      {error && (
        <Alert
          type="error"
          showIcon
          message="加载 Session 运行时失败"
          description={error instanceof Error ? error.message : '未知错误'}
          style={{ marginBottom: 16 }}
        />
      )}

      <Row gutter={[16, 16]} style={{ marginBottom: 24 }}>
        <Col xs={24} sm={12} md={6}>
          <Card>
            <Statistic title="Session 总数" value={stats.totalSessions} prefix={<MessageOutlined />} />
          </Card>
        </Col>
        <Col xs={24} sm={12} md={6}>
          <Card>
            <Statistic title="筛选结果" value={stats.filteredSessions} prefix={<SearchOutlined />} />
          </Card>
        </Col>
        <Col xs={24} sm={12} md={6}>
          <Card>
            <Statistic title="消息总数" value={stats.totalMessages} prefix={<MessageOutlined />} />
          </Card>
        </Col>
        <Col xs={24} sm={12} md={6}>
          <Card>
            <Statistic title="关联 Agent 数" value={stats.boundAgents} prefix={<RobotOutlined />} />
          </Card>
        </Col>
      </Row>

      <Card
        title="Session 运行时"
        extra={
          <Button icon={<ReloadOutlined />} loading={isFetching} onClick={() => void refetch()}>
            刷新
          </Button>
        }
      >
        <Space wrap style={{ marginBottom: 16 }}>
          <Input
            placeholder="搜索 Session / Agent / Conversation"
            value={keyword}
            onChange={(event) => setKeyword(event.target.value)}
            prefix={<SearchOutlined />}
            allowClear
            style={{ width: 320 }}
          />
          <Select
            allowClear
            placeholder="按 Agent 筛选"
            value={agentFilter}
            onChange={(value) => setAgentFilter(value)}
            options={agentOptions}
            style={{ width: 220 }}
          />
        </Space>

        <Table
          rowKey="session_id"
          columns={columns}
          dataSource={filteredSessions}
          loading={isLoading}
          pagination={{ showSizeChanger: true, showTotal: (total) => `共 ${total} 条` }}
        />
      </Card>

      <Drawer
        title={selectedSessionId ? `Session 详情：${selectedSessionId}` : 'Session 详情'}
        width={800}
        open={selectedSessionId !== null}
        onClose={() => setSelectedSessionId(null)}
      >
        {detailError && (
          <Alert
            type="error"
            showIcon
            message="加载 Session 详情失败"
            description={detailError instanceof Error ? detailError.message : '未知错误'}
            style={{ marginBottom: 16 }}
          />
        )}

        {messagesError && (
          <Alert
            type="error"
            showIcon
            message="加载 Session 消息失败"
            description={messagesError instanceof Error ? messagesError.message : '未知错误'}
            style={{ marginBottom: 16 }}
          />
        )}

        {sessionDetail && (
          <Space direction="vertical" size={16} style={{ width: '100%' }}>
            <Descriptions bordered column={1} size="small">
              <Descriptions.Item label="Session ID">
                <Typography.Text code copyable>
                  {sessionDetail.session_id}
                </Typography.Text>
              </Descriptions.Item>
              <Descriptions.Item label="Agent">
                {sessionDetail.agent_id ? <Tag color="blue">{sessionDetail.agent_id}</Tag> : '-'}
              </Descriptions.Item>
              <Descriptions.Item label="Conversation ID">
                {sessionDetail.conversation_id || '-'}
              </Descriptions.Item>
              <Descriptions.Item label="发送者 User ID">
                {sessionDetail.sender_user_id || '-'}
              </Descriptions.Item>
              <Descriptions.Item label="发送者 Staff ID">
                {sessionDetail.sender_staff_id || '-'}
              </Descriptions.Item>
              <Descriptions.Item label="最近 Task ID">
                {sessionDetail.last_task_id || '-'}
              </Descriptions.Item>
              <Descriptions.Item label="消息数">
                {sessionDetail.message_count}
              </Descriptions.Item>
              <Descriptions.Item label="创建时间">
                {formatDateTime(sessionDetail.created_at)}
              </Descriptions.Item>
              <Descriptions.Item label="最近活跃">
                {formatDateTime(sessionDetail.last_active)}
              </Descriptions.Item>
            </Descriptions>

            <Card size="small" title="Session Metadata" loading={isDetailLoading}>
              {Object.keys(sessionDetail.metadata).length > 0 ? (
                <Descriptions bordered column={1} size="small">
                  {Object.entries(sessionDetail.metadata).map(([key, value]) => (
                    <Descriptions.Item key={key} label={key}>
                      <Typography.Text style={{ whiteSpace: 'pre-wrap' }}>
                        {value}
                      </Typography.Text>
                    </Descriptions.Item>
                  ))}
                </Descriptions>
              ) : (
                <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description="暂无元数据" />
              )}
            </Card>

            <Card size="small" title="消息历史" loading={isMessagesLoading}>
              {messages.length > 0 ? (
                <List
                  dataSource={messages}
                  renderItem={(item) => (
                    <List.Item key={`${item.timestamp}-${item.user_message.slice(0, 16)}`}>
                      <Card size="small" style={{ width: '100%' }}>
                        <Space direction="vertical" size={12} style={{ width: '100%' }}>
                          <Tag color="geekblue">{item.timestamp}</Tag>
                          <div>
                            <Typography.Text strong>用户</Typography.Text>
                            <div style={messageBlockStyle}>{item.user_message}</div>
                          </div>
                          <div>
                            <Typography.Text strong>助手</Typography.Text>
                            <div style={messageBlockStyle}>{item.assistant_message}</div>
                          </div>
                        </Space>
                      </Card>
                    </List.Item>
                  )}
                />
              ) : (
                <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description="暂无消息记录" />
              )}
            </Card>
          </Space>
        )}
      </Drawer>
    </div>
  );
};

const messageBlockStyle: React.CSSProperties = {
  marginTop: 8,
  padding: '10px 12px',
  borderRadius: 8,
  background: '#fafafa',
  whiteSpace: 'pre-wrap',
  wordBreak: 'break-word',
};

function formatDateTime(value: string): string {
  const timestamp = Date.parse(value);
  if (Number.isNaN(timestamp)) {
    return value;
  }
  return new Date(timestamp).toLocaleString();
}

export default Sessions;

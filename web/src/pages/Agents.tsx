import React, { useMemo, useState } from 'react';
import {
  Alert,
  Button,
  Card,
  Col,
  Descriptions,
  Drawer,
  Row,
  Space,
  Statistic,
  Table,
  Tag,
  Typography,
} from 'antd';
import {
  AppstoreOutlined,
  EyeOutlined,
  MessageOutlined,
  ReloadOutlined,
  RobotOutlined,
} from '@ant-design/icons';
import { useQuery } from '@tanstack/react-query';
import type { ColumnsType } from 'antd/es/table';

import type { AgentRuntimeSummary } from '../types';
import { agentService } from '../services/agents';

const sourceLayerColorMap: Record<string, string> = {
  global: 'default',
  tenant: 'purple',
  user: 'cyan',
};

function formatAgentSource(sourceLayer: string, sourceScope: string | null): string {
  if (!sourceScope) {
    return sourceLayer;
  }
  return `${sourceLayer} · ${sourceScope}`;
}

const Agents: React.FC = () => {
  const [selectedAgent, setSelectedAgent] = useState<AgentRuntimeSummary | null>(null);

  const {
    data: agents = [],
    isLoading,
    isFetching,
    error,
    refetch,
  } = useQuery({
    queryKey: ['agents-runtime'],
    queryFn: agentService.list,
  });

  const {
    data: agentDetail,
    isLoading: isDetailLoading,
    error: detailError,
  } = useQuery({
    queryKey: [
      'agent-runtime',
      selectedAgent?.agent_id,
      selectedAgent?.source_layer,
      selectedAgent?.source_scope,
    ],
    queryFn: () =>
      agentService.get(selectedAgent!.agent_id, {
        source_layer: selectedAgent!.source_layer,
        source_scope: selectedAgent!.source_scope,
      }),
    enabled: selectedAgent !== null,
  });

  const stats = useMemo(() => {
    const uniqueSkills = new Set(agents.flatMap((agent) => agent.skill_names));
    return {
      totalAgents: agents.length,
      defaultAgents: agents.filter((agent) => agent.is_default).length,
      activeSessions: agents.reduce((sum, agent) => sum + agent.active_session_count, 0),
      uniqueSkills: uniqueSkills.size,
    };
  }, [agents]);

  const columns: ColumnsType<AgentRuntimeSummary> = [
    {
      title: 'Agent',
      dataIndex: 'name',
      key: 'name',
      width: 240,
      render: (_, record) => (
        <Space direction="vertical" size={4}>
          <Space>
            <Typography.Text strong>{record.name}</Typography.Text>
            {record.is_default && <Tag color="gold">默认</Tag>}
          </Space>
          <Typography.Text type="secondary" style={{ fontSize: 12 }}>
            {record.agent_id}
          </Typography.Text>
        </Space>
      ),
    },
    {
      title: '描述',
      dataIndex: 'description',
      key: 'description',
      ellipsis: true,
      render: (value: string) => value || '-',
    },
    {
      title: '已绑定 Skills',
      dataIndex: 'skill_names',
      key: 'skill_names',
      width: 260,
      render: (skillNames: string[]) =>
        skillNames.length > 0 ? (
          <Space size={[4, 4]} wrap>
            {skillNames.map((skillName) => (
              <Tag key={skillName} color="blue">
                {skillName}
              </Tag>
            ))}
          </Space>
        ) : (
          <Tag>无</Tag>
        ),
    },
    {
      title: '活跃 Session',
      dataIndex: 'active_session_count',
      key: 'active_session_count',
      width: 120,
    },
    {
      title: '来源',
      key: 'source',
      width: 220,
      render: (_, record) => (
        <Tag color={sourceLayerColorMap[record.source_layer] ?? 'default'}>
          {formatAgentSource(record.source_layer, record.source_scope)}
        </Tag>
      ),
    },
    {
      title: 'Workspace',
      dataIndex: 'workspace_dir',
      key: 'workspace_dir',
      ellipsis: true,
      render: (value: string) => (
        <Typography.Text code ellipsis={{ tooltip: value }}>
          {value}
        </Typography.Text>
      ),
    },
    {
      title: '操作',
      key: 'actions',
      width: 100,
      render: (_, record) => (
        <Button
          type="link"
          icon={<EyeOutlined />}
          onClick={() => setSelectedAgent(record)}
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
          message="加载 Agent 运行时失败"
          description={error instanceof Error ? error.message : '未知错误'}
          style={{ marginBottom: 16 }}
        />
      )}

      <Row gutter={[16, 16]} style={{ marginBottom: 24 }}>
        <Col xs={24} sm={12} md={6}>
          <Card>
            <Statistic
              title="Agent 数量"
              value={stats.totalAgents}
              prefix={<RobotOutlined />}
            />
          </Card>
        </Col>
        <Col xs={24} sm={12} md={6}>
          <Card>
            <Statistic
              title="默认 Agent"
              value={stats.defaultAgents}
              prefix={<AppstoreOutlined />}
            />
          </Card>
        </Col>
        <Col xs={24} sm={12} md={6}>
          <Card>
            <Statistic
              title="活跃 Session"
              value={stats.activeSessions}
              prefix={<MessageOutlined />}
            />
          </Card>
        </Col>
        <Col xs={24} sm={12} md={6}>
          <Card>
            <Statistic
              title="Skill 种类"
              value={stats.uniqueSkills}
              prefix={<AppstoreOutlined />}
            />
          </Card>
        </Col>
      </Row>

      <Card
        title="Agent 运行时"
        extra={
          <Button icon={<ReloadOutlined />} loading={isFetching} onClick={() => void refetch()}>
            刷新
          </Button>
        }
      >
        <Table
          rowKey={(record) => `${record.agent_id}:${record.source_layer}:${record.source_scope ?? 'global'}`}
          columns={columns}
          dataSource={agents}
          loading={isLoading}
          pagination={{ showSizeChanger: true, showTotal: (total) => `共 ${total} 条` }}
        />
      </Card>

      <Drawer
        title={agentDetail ? `Agent 详情：${agentDetail.name}` : 'Agent 详情'}
        width={720}
        open={selectedAgent !== null}
        onClose={() => setSelectedAgent(null)}
      >
        {detailError && (
          <Alert
            type="error"
            showIcon
            message="加载 Agent 详情失败"
            description={detailError instanceof Error ? detailError.message : '未知错误'}
            style={{ marginBottom: 16 }}
          />
        )}

        {agentDetail && (
          <Space direction="vertical" size={16} style={{ width: '100%' }}>
            <Descriptions column={1} bordered size="small">
              <Descriptions.Item label="Agent ID">{agentDetail.agent_id}</Descriptions.Item>
              <Descriptions.Item label="名称">{agentDetail.name}</Descriptions.Item>
              <Descriptions.Item label="描述">
                {agentDetail.description || '-'}
              </Descriptions.Item>
              <Descriptions.Item label="默认 Agent">
                {agentDetail.is_default ? <Tag color="gold">是</Tag> : <Tag>否</Tag>}
              </Descriptions.Item>
              <Descriptions.Item label="来源">
                <Tag color={sourceLayerColorMap[agentDetail.source_layer] ?? 'default'}>
                  {formatAgentSource(agentDetail.source_layer, agentDetail.source_scope)}
                </Tag>
              </Descriptions.Item>
              <Descriptions.Item label="活跃 Session 数">
                {agentDetail.active_session_count}
              </Descriptions.Item>
              <Descriptions.Item label="Workspace">
                <Typography.Text code copyable>
                  {agentDetail.workspace_dir}
                </Typography.Text>
              </Descriptions.Item>
              <Descriptions.Item label="已绑定 Skills">
                {agentDetail.skill_names.length > 0 ? (
                  <Space size={[4, 4]} wrap>
                    {agentDetail.skill_names.map((skillName) => (
                      <Tag key={skillName} color="blue">
                        {skillName}
                      </Tag>
                    ))}
                  </Space>
                ) : (
                  '-'
                )}
              </Descriptions.Item>
            </Descriptions>

            <Card size="small" title="System Prompt" loading={isDetailLoading}>
              <Typography.Paragraph
                style={{ marginBottom: 0, whiteSpace: 'pre-wrap' }}
              >
                {agentDetail.system_prompt || '-'}
              </Typography.Paragraph>
            </Card>
          </Space>
        )}
      </Drawer>
    </div>
  );
};

export default Agents;

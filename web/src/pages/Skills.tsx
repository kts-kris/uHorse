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
  CheckCircleOutlined,
  ClockCircleOutlined,
  CodeOutlined,
  InfoCircleOutlined,
  ReloadOutlined,
} from '@ant-design/icons';
import { useQuery } from '@tanstack/react-query';
import type { ColumnsType } from 'antd/es/table';

import type { SkillRuntimeSummary } from '../types';
import { skillService } from '../services/agents';

const sourceLayerColorMap: Record<string, string> = {
  global: 'default',
  tenant: 'purple',
  user: 'cyan',
};

function formatSkillSource(sourceLayer: string, sourceScope: string | null): string {
  if (!sourceScope) {
    return sourceLayer;
  }
  return `${sourceLayer} · ${sourceScope}`;
}

const Skills: React.FC = () => {
  const [selectedSkill, setSelectedSkill] = useState<SkillRuntimeSummary | null>(null);

  const {
    data: skills = [],
    isLoading,
    isFetching,
    error,
    refetch,
  } = useQuery({
    queryKey: ['skills-runtime'],
    queryFn: skillService.list,
  });

  const {
    data: skillDetail,
    isLoading: isDetailLoading,
    error: detailError,
  } = useQuery({
    queryKey: [
      'skill-runtime',
      selectedSkill?.name,
      selectedSkill?.source_layer,
      selectedSkill?.source_scope,
    ],
    queryFn: () =>
      skillService.get(selectedSkill!.name, {
        source_layer: selectedSkill!.source_layer,
        source_scope: selectedSkill!.source_scope,
      }),
    enabled: selectedSkill !== null,
  });

  const stats = useMemo(() => {
    const totalTimeout = skills.reduce((sum, skill) => sum + skill.timeout_secs, 0);
    return {
      total: skills.length,
      enabled: skills.filter((skill) => skill.enabled).length,
      processMode: skills.filter((skill) => skill.execution_mode === 'process').length,
      avgTimeout: skills.length === 0 ? 0 : Math.round(totalTimeout / skills.length),
    };
  }, [skills]);

  const columns: ColumnsType<SkillRuntimeSummary> = [
    {
      title: 'Skill',
      dataIndex: 'name',
      key: 'name',
      width: 220,
      render: (_, record) => (
        <Space direction="vertical" size={4}>
          <Space>
            <Typography.Text strong>{record.name}</Typography.Text>
            {record.enabled ? <Tag color="green">启用</Tag> : <Tag>禁用</Tag>}
          </Space>
          <Typography.Text type="secondary" style={{ fontSize: 12 }}>
            {record.version}
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
      title: '执行方式',
      dataIndex: 'execution_mode',
      key: 'execution_mode',
      width: 120,
      render: (value: string) => <Tag color={value === 'process' ? 'blue' : 'default'}>{value}</Tag>,
    },
    {
      title: '来源',
      key: 'source',
      width: 220,
      render: (_, record) => (
        <Tag color={sourceLayerColorMap[record.source_layer] ?? 'default'}>
          {formatSkillSource(record.source_layer, record.source_scope)}
        </Tag>
      ),
    },
    {
      title: '超时',
      dataIndex: 'timeout_secs',
      key: 'timeout_secs',
      width: 100,
      render: (value: number) => `${value}s`,
    },
    {
      title: '权限',
      dataIndex: 'permissions',
      key: 'permissions',
      width: 220,
      render: (permissions: string[]) =>
        permissions.length > 0 ? (
          <Space size={[4, 4]} wrap>
            {permissions.map((permission) => (
              <Tag key={permission}>{permission}</Tag>
            ))}
          </Space>
        ) : (
          <Tag>无</Tag>
        ),
    },
    {
      title: '操作',
      key: 'actions',
      width: 100,
      render: (_, record) => (
        <Button
          type="link"
          icon={<InfoCircleOutlined />}
          onClick={() => setSelectedSkill(record)}
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
          message="加载 Skill 运行时失败"
          description={error instanceof Error ? error.message : '未知错误'}
          style={{ marginBottom: 16 }}
        />
      )}

      <Row gutter={[16, 16]} style={{ marginBottom: 24 }}>
        <Col xs={24} sm={12} md={6}>
          <Card>
            <Statistic title="Skill 总数" value={stats.total} prefix={<CodeOutlined />} />
          </Card>
        </Col>
        <Col xs={24} sm={12} md={6}>
          <Card>
            <Statistic title="已启用" value={stats.enabled} prefix={<CheckCircleOutlined />} />
          </Card>
        </Col>
        <Col xs={24} sm={12} md={6}>
          <Card>
            <Statistic title="Process 模式" value={stats.processMode} prefix={<CodeOutlined />} />
          </Card>
        </Col>
        <Col xs={24} sm={12} md={6}>
          <Card>
            <Statistic title="平均超时" value={stats.avgTimeout} suffix="s" prefix={<ClockCircleOutlined />} />
          </Card>
        </Col>
      </Row>

      <Card
        title="Skill 运行时"
        extra={
          <Button icon={<ReloadOutlined />} loading={isFetching} onClick={() => void refetch()}>
            刷新
          </Button>
        }
      >
        <Table
          rowKey={(record) => `${record.name}:${record.source_layer}:${record.source_scope ?? 'global'}`}
          columns={columns}
          dataSource={skills}
          loading={isLoading}
          pagination={{ showSizeChanger: true, showTotal: (total) => `共 ${total} 条` }}
        />
      </Card>

      <Drawer
        title={skillDetail ? `Skill 详情：${skillDetail.name}` : 'Skill 详情'}
        width={760}
        open={selectedSkill !== null}
        onClose={() => setSelectedSkill(null)}
      >
        {detailError && (
          <Alert
            type="error"
            showIcon
            message="加载 Skill 详情失败"
            description={detailError instanceof Error ? detailError.message : '未知错误'}
            style={{ marginBottom: 16 }}
          />
        )}

        {skillDetail && (
          <Space direction="vertical" size={16} style={{ width: '100%' }}>
            <Descriptions bordered column={1} size="small">
              <Descriptions.Item label="名称">{skillDetail.name}</Descriptions.Item>
              <Descriptions.Item label="描述">
                {skillDetail.description || '-'}
              </Descriptions.Item>
              <Descriptions.Item label="版本">{skillDetail.version}</Descriptions.Item>
              <Descriptions.Item label="作者">{skillDetail.author || '-'}</Descriptions.Item>
              <Descriptions.Item label="状态">
                {skillDetail.enabled ? <Tag color="green">已启用</Tag> : <Tag>已禁用</Tag>}
              </Descriptions.Item>
              <Descriptions.Item label="来源">
                <Tag color={sourceLayerColorMap[skillDetail.source_layer] ?? 'default'}>
                  {formatSkillSource(skillDetail.source_layer, skillDetail.source_scope)}
                </Tag>
              </Descriptions.Item>
              <Descriptions.Item label="执行方式">
                <Tag color={skillDetail.execution_mode === 'process' ? 'blue' : 'default'}>
                  {skillDetail.execution_mode}
                </Tag>
              </Descriptions.Item>
              <Descriptions.Item label="超时">{skillDetail.timeout_secs}s</Descriptions.Item>
              <Descriptions.Item label="最大重试次数">
                {skillDetail.max_retries}
              </Descriptions.Item>
              <Descriptions.Item label="可执行文件">
                {skillDetail.executable ? (
                  <Typography.Text code copyable>
                    {skillDetail.executable}
                  </Typography.Text>
                ) : (
                  '-'
                )}
              </Descriptions.Item>
              <Descriptions.Item label="权限">
                {skillDetail.permissions.length > 0 ? (
                  <Space size={[4, 4]} wrap>
                    {skillDetail.permissions.map((permission) => (
                      <Tag key={permission}>{permission}</Tag>
                    ))}
                  </Space>
                ) : (
                  '-'
                )}
              </Descriptions.Item>
            </Descriptions>

            <Card size="small" title="命令参数" loading={isDetailLoading}>
              <Typography.Paragraph style={{ marginBottom: 0, whiteSpace: 'pre-wrap' }}>
                {skillDetail.args.length > 0 ? JSON.stringify(skillDetail.args, null, 2) : '[]'}
              </Typography.Paragraph>
            </Card>

            <Card size="small" title="环境变量" loading={isDetailLoading}>
              <Typography.Paragraph style={{ marginBottom: 0, whiteSpace: 'pre-wrap' }}>
                {Object.keys(skillDetail.env).length > 0
                  ? JSON.stringify(skillDetail.env, null, 2)
                  : '{}'}
              </Typography.Paragraph>
            </Card>
          </Space>
        )}
      </Drawer>
    </div>
  );
};

export default Skills;

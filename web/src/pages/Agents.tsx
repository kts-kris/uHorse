import React, { useState } from 'react';
import {
  Table,
  Button,
  Space,
  Modal,
  Form,
  Input,
  Switch,
  InputNumber,
  Tag,
  message,
  Popconfirm,
  Card,
} from 'antd';
import {
  PlusOutlined,
  EditOutlined,
  DeleteOutlined,
  PlayCircleOutlined,
  PauseCircleOutlined,
} from '@ant-design/icons';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import type { ColumnsType } from 'antd/es/table';

import { Agent, CreateAgentRequest, UpdateAgentRequest } from '../types';
import { agentsApi } from '../services/agents';

const Agents: React.FC = () => {
  const queryClient = useQueryClient();
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [editingAgent, setEditingAgent] = useState<Agent | null>(null);
  const [form] = Form.useForm();

  // 获取 Agent 列表
  const { data: agents, isLoading } = useQuery({
    queryKey: ['agents'],
    queryFn: agentsApi.listAgents,
  });

  // 创建 Agent
  const createMutation = useMutation({
    mutationFn: agentsApi.createAgent,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['agents'] });
      message.success('Agent 创建成功');
      handleCloseModal();
    },
    onError: () => {
      message.error('Agent 创建失败');
    },
  });

  // 更新 Agent
  const updateMutation = useMutation({
    mutationFn: ({ id, data }: { id: string; data: UpdateAgentRequest }) =>
      agentsApi.updateAgent(id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['agents'] });
      message.success('Agent 更新成功');
      handleCloseModal();
    },
    onError: () => {
      message.error('Agent 更新失败');
    },
  });

  // 删除 Agent
  const deleteMutation = useMutation({
    mutationFn: agentsApi.deleteAgent,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['agents'] });
      message.success('Agent 删除成功');
    },
    onError: () => {
      message.error('Agent 删除失败');
    },
  });

  // 切换启用状态
  const toggleMutation = useMutation({
    mutationFn: ({ id, enabled }: { id: string; enabled: boolean }) =>
      agentsApi.updateAgent(id, { enabled }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['agents'] });
      message.success('状态更新成功');
    },
  });

  const handleOpenModal = (agent?: Agent) => {
    if (agent) {
      setEditingAgent(agent);
      form.setFieldsValue(agent);
    } else {
      setEditingAgent(null);
      form.resetFields();
    }
    setIsModalOpen(true);
  };

  const handleCloseModal = () => {
    setIsModalOpen(false);
    setEditingAgent(null);
    form.resetFields();
  };

  const handleSubmit = async () => {
    const values = await form.validateFields();
    if (editingAgent) {
      updateMutation.mutate({ id: editingAgent.id, data: values });
    } else {
      createMutation.mutate(values as CreateAgentRequest);
    }
  };

  const columns: ColumnsType<Agent> = [
    {
      title: '名称',
      dataIndex: 'name',
      key: 'name',
      width: 150,
    },
    {
      title: '描述',
      dataIndex: 'description',
      key: 'description',
      ellipsis: true,
    },
    {
      title: '模型',
      dataIndex: 'model',
      key: 'model',
      width: 150,
      render: (model) => <Tag color="blue">{model}</Tag>,
    },
    {
      title: '温度',
      dataIndex: 'temperature',
      key: 'temperature',
      width: 80,
      render: (temp) => temp?.toFixed(2),
    },
    {
      title: '状态',
      dataIndex: 'enabled',
      key: 'enabled',
      width: 100,
      render: (enabled, record) => (
        <Switch
          checked={enabled}
          onChange={(checked) =>
            toggleMutation.mutate({ id: record.id, enabled: checked })
          }
          checkedChildren={<PlayCircleOutlined />}
          unCheckedChildren={<PauseCircleOutlined />}
        />
      ),
    },
    {
      title: '创建时间',
      dataIndex: 'created_at',
      key: 'created_at',
      width: 180,
      render: (time) => new Date(time).toLocaleString(),
    },
    {
      title: '操作',
      key: 'actions',
      width: 150,
      render: (_, record) => (
        <Space>
          <Button
            type="link"
            icon={<EditOutlined />}
            onClick={() => handleOpenModal(record)}
          >
            编辑
          </Button>
          <Popconfirm
            title="确定要删除此 Agent 吗？"
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
    <Card
      title="Agent 管理"
      extra={
        <Button
          type="primary"
          icon={<PlusOutlined />}
          onClick={() => handleOpenModal()}
        >
          创建 Agent
        </Button>
      }
    >
      <Table
        columns={columns}
        dataSource={agents}
        rowKey="id"
        loading={isLoading}
        pagination={{
          showSizeChanger: true,
          showQuickJumper: true,
          showTotal: (total) => `共 ${total} 条`,
        }}
      />

      <Modal
        title={editingAgent ? '编辑 Agent' : '创建 Agent'}
        open={isModalOpen}
        onOk={handleSubmit}
        onCancel={handleCloseModal}
        width={600}
        confirmLoading={createMutation.isPending || updateMutation.isPending}
      >
        <Form form={form} layout="vertical">
          <Form.Item
            name="name"
            label="名称"
            rules={[{ required: true, message: '请输入 Agent 名称' }]}
          >
            <Input placeholder="例如：客服助手" />
          </Form.Item>

          <Form.Item name="description" label="描述">
            <Input.TextArea rows={2} placeholder="Agent 的功能描述" />
          </Form.Item>

          <Form.Item
            name="model"
            label="模型"
            rules={[{ required: true, message: '请选择模型' }]}
            initialValue="gpt-4"
          >
            <Input placeholder="例如：gpt-4、claude-3-opus" />
          </Form.Item>

          <Form.Item name="system_prompt" label="系统提示词">
            <Input.TextArea
              rows={4}
              placeholder="定义 Agent 的行为和角色..."
            />
          </Form.Item>

          <Form.Item label="参数设置">
            <Space>
              <Form.Item
                name="temperature"
                label="温度"
                initialValue={0.7}
                noStyle
              >
                <InputNumber min={0} max={2} step={0.1} />
              </Form.Item>
              <Form.Item
                name="max_tokens"
                label="最大 Token"
                initialValue={4096}
                noStyle
              >
                <InputNumber min={100} max={128000} step={100} />
              </Form.Item>
            </Space>
          </Form.Item>
        </Form>
      </Modal>
    </Card>
  );
};

export default Agents;

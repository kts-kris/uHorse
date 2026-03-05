import React, { useState } from 'react';
import {
  Table,
  Button,
  Space,
  Modal,
  Form,
  Input,
  Switch,
  Tag,
  message,
  Popconfirm,
  Card,
  Tabs,
  Descriptions,
  Collapse,
} from 'antd';
import {
  PlusOutlined,
  EditOutlined,
  DeleteOutlined,
  PlayCircleOutlined,
  CodeOutlined,
  InfoCircleOutlined,
} from '@ant-design/icons';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import type { ColumnsType } from 'antd/es/table';

import { Skill, CreateSkillRequest, SkillParameter } from '../types';

// 模拟 API（实际应从 services 导入）
const skillsApi = {
  listSkills: async (): Promise<Skill[]> => {
    const response = await fetch('/api/v1/skills');
    if (!response.ok) throw new Error('Failed to fetch skills');
    return response.json();
  },
  createSkill: async (data: CreateSkillRequest): Promise<Skill> => {
    const response = await fetch('/api/v1/skills', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(data),
    });
    if (!response.ok) throw new Error('Failed to create skill');
    return response.json();
  },
  updateSkill: async (id: string, data: Partial<Skill>): Promise<Skill> => {
    const response = await fetch(`/api/v1/skills/${id}`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(data),
    });
    if (!response.ok) throw new Error('Failed to update skill');
    return response.json();
  },
  deleteSkill: async (id: string): Promise<void> => {
    const response = await fetch(`/api/v1/skills/${id}`, { method: 'DELETE' });
    if (!response.ok) throw new Error('Failed to delete skill');
  },
};

const Skills: React.FC = () => {
  const queryClient = useQueryClient();
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [isDetailOpen, setIsDetailOpen] = useState(false);
  const [editingSkill, setEditingSkill] = useState<Skill | null>(null);
  const [selectedSkill, setSelectedSkill] = useState<Skill | null>(null);
  const [form] = Form.useForm();

  // 获取技能列表
  const { data: skills, isLoading } = useQuery({
    queryKey: ['skills'],
    queryFn: skillsApi.listSkills,
  });

  // 创建技能
  const createMutation = useMutation({
    mutationFn: skillsApi.createSkill,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['skills'] });
      message.success('技能创建成功');
      handleCloseModal();
    },
    onError: () => message.error('技能创建失败'),
  });

  // 更新技能
  const updateMutation = useMutation({
    mutationFn: ({ id, data }: { id: string; data: Partial<Skill> }) =>
      skillsApi.updateSkill(id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['skills'] });
      message.success('技能更新成功');
      handleCloseModal();
    },
    onError: () => message.error('技能更新失败'),
  });

  // 删除技能
  const deleteMutation = useMutation({
    mutationFn: skillsApi.deleteSkill,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['skills'] });
      message.success('技能删除成功');
    },
    onError: () => message.error('技能删除失败'),
  });

  // 切换启用状态
  const toggleMutation = useMutation({
    mutationFn: ({ id, enabled }: { id: string; enabled: boolean }) =>
      skillsApi.updateSkill(id, { enabled }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['skills'] });
      message.success('状态更新成功');
    },
  });

  const handleOpenModal = (skill?: Skill) => {
    if (skill) {
      setEditingSkill(skill);
      form.setFieldsValue({
        ...skill,
        skill_content: JSON.stringify(skill.parameters, null, 2),
      });
    } else {
      setEditingSkill(null);
      form.resetFields();
    }
    setIsModalOpen(true);
  };

  const handleCloseModal = () => {
    setIsModalOpen(false);
    setEditingSkill(null);
    form.resetFields();
  };

  const handleViewDetail = (skill: Skill) => {
    setSelectedSkill(skill);
    setIsDetailOpen(true);
  };

  const handleSubmit = async () => {
    const values = await form.validateFields();
    const skillData: CreateSkillRequest = {
      name: values.name,
      description: values.description,
      version: values.version || '1.0.0',
      author: values.author,
      skill_content: values.skill_content,
    };
    if (editingSkill) {
      updateMutation.mutate({ id: editingSkill.id, data: skillData });
    } else {
      createMutation.mutate(skillData);
    }
  };

  const renderParameterType = (type: string) => {
    const colors: Record<string, string> = {
      string: 'blue',
      number: 'green',
      boolean: 'orange',
      object: 'purple',
      array: 'cyan',
    };
    return <Tag color={colors[type] || 'default'}>{type}</Tag>;
  };

  const columns: ColumnsType<Skill> = [
    {
      title: '名称',
      dataIndex: 'name',
      key: 'name',
      width: 150,
      render: (name) => <Tag color="geekblue">{name}</Tag>,
    },
    {
      title: '描述',
      dataIndex: 'description',
      key: 'description',
      ellipsis: true,
    },
    {
      title: '版本',
      dataIndex: 'version',
      key: 'version',
      width: 100,
      render: (v) => <Tag>{v}</Tag>,
    },
    {
      title: '作者',
      dataIndex: 'author',
      key: 'author',
      width: 120,
    },
    {
      title: '参数数量',
      dataIndex: 'parameters',
      key: 'parameters',
      width: 100,
      render: (params: SkillParameter[]) => params?.length || 0,
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
          unCheckedChildren={<PlayCircleOutlined />}
        />
      ),
    },
    {
      title: '操作',
      key: 'actions',
      width: 200,
      render: (_, record) => (
        <Space>
          <Button
            type="link"
            icon={<InfoCircleOutlined />}
            onClick={() => handleViewDetail(record)}
          >
            详情
          </Button>
          <Button
            type="link"
            icon={<EditOutlined />}
            onClick={() => handleOpenModal(record)}
          >
            编辑
          </Button>
          <Popconfirm
            title="确定要删除此技能吗？"
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
      title="技能管理"
      extra={
        <Button
          type="primary"
          icon={<PlusOutlined />}
          onClick={() => handleOpenModal()}
        >
          创建技能
        </Button>
      }
    >
      <Table
        columns={columns}
        dataSource={skills}
        rowKey="id"
        loading={isLoading}
        pagination={{
          showSizeChanger: true,
          showQuickJumper: true,
          showTotal: (total) => `共 ${total} 条`,
        }}
      />

      {/* 创建/编辑 Modal */}
      <Modal
        title={editingSkill ? '编辑技能' : '创建技能'}
        open={isModalOpen}
        onOk={handleSubmit}
        onCancel={handleCloseModal}
        width={700}
        confirmLoading={createMutation.isPending || updateMutation.isPending}
      >
        <Form form={form} layout="vertical">
          <Form.Item
            name="name"
            label="技能名称"
            rules={[{ required: true, message: '请输入技能名称' }]}
          >
            <Input placeholder="例如：weather_query" />
          </Form.Item>

          <Form.Item name="description" label="描述">
            <Input.TextArea rows={2} placeholder="技能的功能描述" />
          </Form.Item>

          <Space>
            <Form.Item
              name="version"
              label="版本"
              initialValue="1.0.0"
              rules={[{ required: true }]}
            >
              <Input placeholder="1.0.0" />
            </Form.Item>
            <Form.Item name="author" label="作者">
              <Input placeholder="作者名称" />
            </Form.Item>
          </Space>

          <Form.Item
            name="skill_content"
            label="技能内容"
            rules={[{ required: true, message: '请输入技能内容' }]}
          >
            <Input.TextArea
              rows={10}
              placeholder="技能的 YAML/JSON 定义内容..."
              style={{ fontFamily: 'monospace' }}
            />
          </Form.Item>
        </Form>
      </Modal>

      {/* 详情 Modal */}
      <Modal
        title="技能详情"
        open={isDetailOpen}
        onCancel={() => setIsDetailOpen(false)}
        footer={null}
        width={700}
      >
        {selectedSkill && (
          <Tabs
            items={[
              {
                key: 'info',
                label: '基本信息',
                children: (
                  <Descriptions column={2} bordered>
                    <Descriptions.Item label="名称">
                      {selectedSkill.name}
                    </Descriptions.Item>
                    <Descriptions.Item label="版本">
                      {selectedSkill.version}
                    </Descriptions.Item>
                    <Descriptions.Item label="作者">
                      {selectedSkill.author || '-'}
                    </Descriptions.Item>
                    <Descriptions.Item label="状态">
                      <Tag color={selectedSkill.enabled ? 'green' : 'default'}>
                        {selectedSkill.enabled ? '已启用' : '已禁用'}
                      </Tag>
                    </Descriptions.Item>
                    <Descriptions.Item label="描述" span={2}>
                      {selectedSkill.description || '-'}
                    </Descriptions.Item>
                  </Descriptions>
                ),
              },
              {
                key: 'params',
                label: '参数定义',
                children: (
                  <Collapse
                    items={(selectedSkill.parameters || []).map((param, idx) => ({
                      key: idx,
                      label: (
                        <Space>
                          <Tag>{param.name}</Tag>
                          {renderParameterType(param.type)}
                          {param.required && <Tag color="red">必填</Tag>}
                        </Space>
                      ),
                      children: (
                        <Descriptions column={1} size="small">
                          <Descriptions.Item label="类型">
                            {param.type}
                          </Descriptions.Item>
                          <Descriptions.Item label="描述">
                            {param.description || '-'}
                          </Descriptions.Item>
                          <Descriptions.Item label="默认值">
                            {param.default !== undefined
                              ? JSON.stringify(param.default)
                              : '-'}
                          </Descriptions.Item>
                        </Descriptions>
                      ),
                    }))}
                  />
                ),
              },
            ]}
          />
        )}
      </Modal>
    </Card>
  );
};

export default Skills;

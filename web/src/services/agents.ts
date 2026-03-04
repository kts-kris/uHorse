import client from './api';
import type { ApiResponse, PaginatedResponse } from './api';
import type { Agent, CreateAgentRequest, UpdateAgentRequest } from '../types';

export const agentService = {
  // 获取所有 Agent
  async list(page = 1, pageSize = 20): Promise<PaginatedResponse<Agent>> {
    const response = await client.get<ApiResponse<PaginatedResponse<Agent>>>(
      `/api/v1/agents?page=${page}&page_size=${pageSize}`
    );
    if (response.data.success) return response.data.data;
    throw new Error(response.data.error?.message || '获取 Agent 列表失败');
  },

  // 获取单个 Agent
  async get(id: string): Promise<Agent> {
    const response = await client.get<ApiResponse<Agent>>(`/api/v1/agents/${id}`);
    if (response.data.success) return response.data.data;
    throw new Error(response.data.error?.message || '获取 Agent 失败');
  },

  // 创建 Agent
  async create(data: CreateAgentRequest): Promise<Agent> {
    const response = await client.post<ApiResponse<Agent>>('/api/v1/agents', data);
    if (response.data.success) return response.data.data;
    throw new Error(response.data.error?.message || '创建 Agent 失败');
  },

  // 更新 Agent
  async update(id: string, data: UpdateAgentRequest): Promise<Agent> {
    const response = await client.put<ApiResponse<Agent>>(`/api/v1/agents/${id}`, data);
    if (response.data.success) return response.data.data;
    throw new Error(response.data.error?.message || '更新 Agent 失败');
  },

  // 删除 Agent
  async delete(id: string): Promise<void> {
    await client.delete(`/api/v1/agents/${id}`);
  },

  // 启用/禁用 Agent
  async toggle(id: string, enabled: boolean): Promise<Agent> {
    return this.update(id, { enabled });
  },
};

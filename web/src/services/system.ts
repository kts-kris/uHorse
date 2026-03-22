import client from './api';
import { getApiErrorMessage } from './api';
import type { ApiResponse } from './api';
import type { HubStats, NodeRuntimeInfo, TaskRuntimeInfo } from '../types';

export const systemService = {
  async getStats(): Promise<HubStats> {
    const { data } = await client.get<ApiResponse<HubStats>>('/api/stats');
    if (data.success && data.data !== null) return data.data;
    throw new Error(getApiErrorMessage(data.error, '获取 Hub 统计失败'));
  },

  async getNodes(): Promise<NodeRuntimeInfo[]> {
    const { data } = await client.get<ApiResponse<NodeRuntimeInfo[]>>('/api/nodes');
    if (data.success && data.data !== null) return data.data;
    throw new Error(getApiErrorMessage(data.error, '获取节点列表失败'));
  },

  async getTasks(): Promise<TaskRuntimeInfo[]> {
    const { data } = await client.get<ApiResponse<TaskRuntimeInfo[]>>('/api/tasks');
    if (data.success && data.data !== null) return data.data;
    throw new Error(getApiErrorMessage(data.error, '获取任务列表失败'));
  },
};

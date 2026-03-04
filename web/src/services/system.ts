import client from './api';
import type { ApiResponse } from './api';
import type { SystemInfo, SystemMetrics, ChannelStatus } from '../types';

export const systemService = {
  // 获取系统信息
  async getInfo(): Promise<SystemInfo> {
    const response = await client.get<ApiResponse<SystemInfo>>('/api/v1/system/info');
    if (response.data.success) return response.data.data;
    throw new Error(response.data.error?.message || '获取系统信息失败');
  },

  // 获取系统指标
  async getMetrics(): Promise<SystemMetrics> {
    const response = await client.get<ApiResponse<SystemMetrics>>('/api/v1/system/metrics');
    if (response.data.success) return response.data.data;
    throw new Error(response.data.error?.message || '获取系统指标失败');
  },

  // 获取通道状态
  async getChannels(): Promise<ChannelStatus[]> {
    const response = await client.get<ApiResponse<ChannelStatus[]>>('/api/v1/channels');
    if (response.data.success) return response.data.data;
    throw new Error(response.data.error?.message || '获取通道状态失败');
  },

  // 获取单个通道状态
  async getChannelStatus(channelType: string): Promise<ChannelStatus> {
    const response = await client.get<ApiResponse<ChannelStatus>>(
      `/api/v1/channels/${channelType}/status`
    );
    if (response.data.success) return response.data.data;
    throw new Error(response.data.error?.message || '获取通道状态失败');
  },
};

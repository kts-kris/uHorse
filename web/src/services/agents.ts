import client from './api';
import { getApiErrorMessage } from './api';
import type { ApiResponse } from './api';
import type {
  AgentRuntimeDetail,
  AgentRuntimeSummary,
  SessionMessageRecord,
  SessionRuntimeDetail,
  SessionRuntimeSummary,
  SkillRuntimeDetail,
  SkillRuntimeSummary,
} from '../types';

export const agentService = {
  async list(): Promise<AgentRuntimeSummary[]> {
    const { data } = await client.get<ApiResponse<AgentRuntimeSummary[]>>('/api/v1/agents');
    if (data.success && data.data !== null) return data.data;
    throw new Error(getApiErrorMessage(data.error, '获取 Agent 运行时列表失败'));
  },

  async get(
    agentId: string,
    options?: { source_layer?: string; source_scope?: string | null }
  ): Promise<AgentRuntimeDetail> {
    const { data } = await client.get<ApiResponse<AgentRuntimeDetail>>(
      `/api/v1/agents/${encodeURIComponent(agentId)}`,
      {
        params: {
          source_layer: options?.source_layer,
          source_scope: options?.source_scope ?? undefined,
        },
      }
    );
    if (data.success && data.data !== null) return data.data;
    throw new Error(getApiErrorMessage(data.error, '获取 Agent 详情失败'));
  },
};

export const skillService = {
  async list(): Promise<SkillRuntimeSummary[]> {
    const { data } = await client.get<ApiResponse<SkillRuntimeSummary[]>>('/api/v1/skills');
    if (data.success && data.data !== null) return data.data;
    throw new Error(getApiErrorMessage(data.error, '获取 Skill 列表失败'));
  },

  async get(
    skillName: string,
    options?: { source_layer?: string; source_scope?: string | null }
  ): Promise<SkillRuntimeDetail> {
    const { data } = await client.get<ApiResponse<SkillRuntimeDetail>>(
      `/api/v1/skills/${encodeURIComponent(skillName)}`,
      {
        params: {
          source_layer: options?.source_layer,
          source_scope: options?.source_scope ?? undefined,
        },
      }
    );
    if (data.success && data.data !== null) return data.data;
    throw new Error(getApiErrorMessage(data.error, '获取 Skill 详情失败'));
  },
};

export const sessionService = {
  async list(): Promise<SessionRuntimeSummary[]> {
    const { data } = await client.get<ApiResponse<SessionRuntimeSummary[]>>('/api/v1/sessions');
    if (data.success && data.data !== null) return data.data;
    throw new Error(getApiErrorMessage(data.error, '获取 Session 列表失败'));
  },

  async get(sessionId: string): Promise<SessionRuntimeDetail> {
    const { data } = await client.get<ApiResponse<SessionRuntimeDetail>>(
      `/api/v1/sessions/${encodeURIComponent(sessionId)}`
    );
    if (data.success && data.data !== null) return data.data;
    throw new Error(getApiErrorMessage(data.error, '获取 Session 详情失败'));
  },

  async getMessages(sessionId: string): Promise<SessionMessageRecord[]> {
    const { data } = await client.get<ApiResponse<SessionMessageRecord[]>>(
      `/api/v1/sessions/${encodeURIComponent(sessionId)}/messages`
    );
    if (data.success && data.data !== null) return data.data;
    throw new Error(getApiErrorMessage(data.error, '获取 Session 消息失败'));
  },
};

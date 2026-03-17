/**
 * uHorse SDK Types
 */

import { z } from 'zod';

// Agent
export const AgentSchema = z.object({
  id: z.string(),
  name: z.string(),
  description: z.string().optional(),
  channel: z.string().optional(),
  status: z.enum(['active', 'inactive', 'error']).default('active'),
  createdAt: z.string().optional(),
  updatedAt: z.string().optional(),
});

export type Agent = z.infer<typeof AgentSchema>;

// Session
export const SessionSchema = z.object({
  id: z.string(),
  agentId: z.string(),
  channel: z.string(),
  userId: z.string().optional(),
  status: z.enum(['active', 'closed', 'error']).default('active'),
  createdAt: z.string().optional(),
  updatedAt: z.string().optional(),
  metadata: z.record(z.unknown()).default({}),
});

export type Session = z.infer<typeof SessionSchema>;

// Tool Call
export const ToolCallSchema = z.object({
  id: z.string(),
  name: z.string(),
  parameters: z.record(z.unknown()).default({}),
  result: z.unknown().optional(),
  status: z.enum(['pending', 'success', 'error']).default('pending'),
  error: z.string().optional(),
});

export type ToolCall = z.infer<typeof ToolCallSchema>;

// Message
export const MessageSchema = z.object({
  id: z.string(),
  sessionId: z.string(),
  role: z.enum(['user', 'assistant', 'tool', 'system']),
  content: z.string(),
  toolCalls: z.array(ToolCallSchema).optional(),
  createdAt: z.string().optional(),
  metadata: z.record(z.unknown()).default({}),
});

export type Message = z.infer<typeof MessageSchema>;

// Skill
export const SkillSchema = z.object({
  name: z.string(),
  description: z.string().optional(),
  version: z.string().optional(),
  parameters: z.record(z.unknown()).default({}),
  enabled: z.boolean().default(true),
});

export type Skill = z.infer<typeof SkillSchema>;

// Error Response
export const ErrorResponseSchema = z.object({
  error: z.string(),
  message: z.string(),
  details: z.record(z.unknown()).optional(),
});

export type ErrorResponse = z.infer<typeof ErrorResponseSchema>;

// Pagination
export const PaginationParamsSchema = z.object({
  limit: z.number().optional(),
  offset: z.number().optional(),
});

export type PaginationParams = z.infer<typeof PaginationParamsSchema>;

/**
 * uHorse API Client
 */

import { Agent, Session, Message, Skill, ErrorResponse } from './types';
import { UHorseError, AuthenticationError, NotFoundError, ValidationError, RateLimitError } from './errors';

export interface ClientOptions {
  /** uHorse server URL */
  baseUrl: string;
  /** API key for authentication */
  apiKey?: string;
  /** Request timeout in milliseconds */
  timeout?: number;
}

/**
 * uHorse API Client
 */
export class Client {
  private readonly baseUrl: string;
  private readonly apiKey?: string;
  private readonly timeout: number;

  /** Agent operations */
  readonly agents: AgentsClient;
  /** Session operations */
  readonly sessions: SessionsClient;
  /** Chat operations */
  readonly chat: ChatClient;
  /** Skill operations */
  readonly skills: SkillsClient;

  constructor(options: ClientOptions) {
    this.baseUrl = options.baseUrl.replace(/\/$/, '');
    this.apiKey = options.apiKey;
    this.timeout = options.timeout ?? 30000;

    this.agents = new AgentsClient(this);
    this.sessions = new SessionsClient(this);
    this.chat = new ChatClient(this);
    this.skills = new SkillsClient(this);
  }

  /**
   * Make an HTTP request
   */
  async request<T>(
    method: string,
    path: string,
    options?: {
      body?: unknown;
      params?: Record<string, string>;
    }
  ): Promise<T> {
    const url = new URL(`${this.baseUrl}${path}`);

    if (options?.params) {
      Object.entries(options.params).forEach(([key, value]) => {
        url.searchParams.set(key, value);
      });
    }

    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
    };

    if (this.apiKey) {
      headers['Authorization'] = `Bearer ${this.apiKey}`;
    }

    const response = await fetch(url.toString(), {
      method,
      headers,
      body: options?.body ? JSON.stringify(options.body) : undefined,
      signal: AbortSignal.timeout(this.timeout),
    });

    if (!response.ok) {
      await this.handleError(response);
    }

    return response.json();
  }

  private async handleError(response: Response): Promise<never> {
    let error: ErrorResponse;

    try {
      error = await response.json();
    } catch {
      error = { error: 'unknown', message: response.statusText };
    }

    switch (response.status) {
      case 401:
        throw new AuthenticationError(error.message);
      case 404:
        throw new NotFoundError(error.message);
      case 422:
        throw new ValidationError(error.message);
      case 429:
        throw new RateLimitError(error.message);
      default:
        throw new UHorseError(error.message, error.error);
    }
  }
}

/**
 * Agents API client
 */
class AgentsClient {
  constructor(private client: Client) {}

  /**
   * List all agents
   */
  async list(): Promise<Agent[]> {
    const data = await this.client.request<{ agents: Agent[] }>('GET', '/api/v1/agents');
    return data.agents ?? [];
  }

  /**
   * Get agent by ID
   */
  async get(agentId: string): Promise<Agent> {
    return this.client.request<Agent>('GET', `/api/v1/agents/${agentId}`);
  }

  /**
   * Create a new agent
   */
  async create(options: {
    name: string;
    description?: string;
    channel?: string;
  }): Promise<Agent> {
    return this.client.request<Agent>('POST', '/api/v1/agents', { body: options });
  }

  /**
   * Delete an agent
   */
  async delete(agentId: string): Promise<void> {
    await this.client.request('DELETE', `/api/v1/agents/${agentId}`);
  }
}

/**
 * Sessions API client
 */
class SessionsClient {
  constructor(private client: Client) {}

  /**
   * List sessions
   */
  async list(options?: {
    agentId?: string;
    channel?: string;
  }): Promise<Session[]> {
    const params: Record<string, string> = {};
    if (options?.agentId) params.agent_id = options.agentId;
    if (options?.channel) params.channel = options.channel;

    const data = await this.client.request<{ sessions: Session[] }>(
      'GET',
      '/api/v1/sessions',
      { params }
    );
    return data.sessions ?? [];
  }

  /**
   * Get session by ID
   */
  async get(sessionId: string): Promise<Session> {
    return this.client.request<Session>('GET', `/api/v1/sessions/${sessionId}`);
  }

  /**
   * Create a new session
   */
  async create(options: {
    agentId: string;
    channel: string;
    userId?: string;
  }): Promise<Session> {
    return this.client.request<Session>('POST', '/api/v1/sessions', { body: options });
  }

  /**
   * Delete a session
   */
  async delete(sessionId: string): Promise<void> {
    await this.client.request('DELETE', `/api/v1/sessions/${sessionId}`);
  }
}

/**
 * Chat API client
 */
class ChatClient {
  constructor(private client: Client) {}

  /**
   * Send a message
   */
  async send(options: {
    sessionId: string;
    message: string;
    metadata?: Record<string, unknown>;
  }): Promise<Message> {
    return this.client.request<Message>('POST', '/api/v1/chat/messages', {
      body: {
        session_id: options.sessionId,
        content: options.message,
        metadata: options.metadata,
      },
    });
  }

  /**
   * Get message history
   */
  async history(sessionId: string, limit = 50): Promise<Message[]> {
    const data = await this.client.request<{ messages: Message[] }>(
      'GET',
      `/api/v1/sessions/${sessionId}/messages`,
      { params: { limit: String(limit) } }
    );
    return data.messages ?? [];
  }

  /**
   * Stream a chat response
   */
  async *stream(options: {
    sessionId: string;
    message: string;
  }): AsyncGenerator<string> {
    const wsUrl = this.client['baseUrl'].replace(/^http/, 'ws') + '/api/v1/chat/stream';

    const ws = new WebSocket(wsUrl);

    // Wait for connection
    await new Promise<void>((resolve, reject) => {
      ws.onopen = () => resolve();
      ws.onerror = (err) => reject(err);
    });

    // Send message
    ws.send(JSON.stringify({
      session_id: options.sessionId,
      content: options.message,
    }));

    // Receive chunks
    yield* new AsyncGenerator<string>(async function* () {
      while (true) {
        const event = await new Promise<{ type: string; content?: string; message?: string }>(
          (resolve, reject) => {
            ws.onmessage = (e) => resolve(JSON.parse(e.data.toString()));
            ws.onerror = (err) => reject(err);
          }
        );

        if (event.type === 'chunk' && event.content) {
          yield event.content;
        } else if (event.type === 'done') {
          break;
        } else if (event.type === 'error') {
          throw new UHorseError(event.message ?? 'Stream error', 'stream_error');
        }
      }
    }());

    ws.close();
  }
}

/**
 * Skills API client
 */
class SkillsClient {
  constructor(private client: Client) {}

  /**
   * List all skills
   */
  async list(): Promise<Skill[]> {
    const data = await this.client.request<{ skills: Skill[] }>('GET', '/api/v1/skills');
    return data.skills ?? [];
  }

  /**
   * Get skill by name
   */
  async get(skillName: string): Promise<Skill> {
    return this.client.request<Skill>('GET', `/api/v1/skills/${skillName}`);
  }

  /**
   * Execute a skill
   */
  async execute<T = unknown>(skillName: string, parameters: Record<string, unknown>): Promise<T> {
    const data = await this.client.request<{ result: T }>(
      'POST',
      `/api/v1/skills/${skillName}/execute`,
      { body: { parameters } }
    );
    return data.result;
  }
}

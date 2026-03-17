/**
 * uHorse TypeScript SDK
 *
 * A TypeScript SDK for interacting with uHorse AI Gateway.
 *
 * @example
 * ```typescript
 * import { Client } from '@uhorse/sdk';
 *
 * const client = new Client({
 *   baseUrl: 'http://localhost:8080',
 *   apiKey: 'your-api-key',
 * });
 *
 * // List agents
 * const agents = await client.agents.list();
 *
 * // Send message
 * const response = await client.chat.send({
 *   sessionId: 'session-123',
 *   message: 'Hello!',
 * });
 * ```
 */

export { Client, ClientOptions } from './client';
export * from './types';
export * from './errors';

// Version
export const VERSION = '0.1.0';

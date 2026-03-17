/**
 * uHorse SDK Errors
 */

/**
 * Base error class for uHorse SDK
 */
export class UHorseError extends Error {
  constructor(
    message: string,
    public readonly code: string = 'unknown_error'
  ) {
    super(message);
    this.name = 'UHorseError';
  }

  override toString(): string {
    return `[${this.code}] ${this.message}`;
  }
}

/**
 * Authentication failed
 */
export class AuthenticationError extends UHorseError {
  constructor(message: string = 'Authentication failed') {
    super(message, 'authentication_error');
    this.name = 'AuthenticationError';
  }
}

/**
 * Resource not found
 */
export class NotFoundError extends UHorseError {
  constructor(message: string = 'Resource not found') {
    super(message, 'not_found');
    this.name = 'NotFoundError';
  }
}

/**
 * Validation error
 */
export class ValidationError extends UHorseError {
  constructor(message: string = 'Validation error') {
    super(message, 'validation_error');
    this.name = 'ValidationError';
  }
}

/**
 * Rate limit exceeded
 */
export class RateLimitError extends UHorseError {
  constructor(message: string = 'Rate limit exceeded') {
    super(message, 'rate_limit_exceeded');
    this.name = 'RateLimitError';
  }
}

/**
 * Connection error
 */
export class ConnectionError extends UHorseError {
  constructor(message: string = 'Connection failed') {
    super(message, 'connection_error');
    this.name = 'ConnectionError';
  }
}

/**
 * Request timeout
 */
export class TimeoutError extends UHorseError {
  constructor(message: string = 'Request timeout') {
    super(message, 'timeout');
    this.name = 'TimeoutError';
  }
}

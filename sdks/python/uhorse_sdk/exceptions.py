"""
uHorse SDK Exceptions
"""


class UHorseError(Exception):
    """Base exception for uHorse SDK."""

    def __init__(self, message: str, code: Optional[str] = None):
        self.message = message
        self.code = code or "unknown_error"
        super().__init__(self.message)

    def __str__(self):
        return f"[{self.code}] {self.message}"


class AuthenticationError(UHorseError):
    """Authentication failed."""

    def __init__(self, message: str = "Authentication failed"):
        super().__init__(message, "authentication_error")


class NotFoundError(UHorseError):
    """Resource not found."""

    def __init__(self, message: str = "Resource not found"):
        super().__init__(message, "not_found")


class ValidationError(UHorseError):
    """Validation error."""

    def __init__(self, message: str = "Validation error"):
        super().__init__(message, "validation_error")


class RateLimitError(UHorseError):
    """Rate limit exceeded."""

    def __init__(self, message: str = "Rate limit exceeded"):
        super().__init__(message, "rate_limit_exceeded")


class ConnectionError(UHorseError):
    """Connection error."""

    def __init__(self, message: str = "Connection failed"):
        super().__init__(message, "connection_error")


class TimeoutError(UHorseError):
    """Request timeout."""

    def __init__(self, message: str = "Request timeout"):
        super().__init__(message, "timeout")

"""
uHorse Python SDK

A Python SDK for interacting with uHorse AI Gateway.
"""

from .client import Client
from .types import (
    Agent,
    Session,
    Message,
    ToolCall,
    Skill,
    ErrorResponse,
)
from .exceptions import (
    UHorseError,
    AuthenticationError,
    NotFoundError,
    ValidationError,
    RateLimitError,
)

__version__ = "0.1.0"
__all__ = [
    "Client",
    "Agent",
    "Session",
    "Message",
    "ToolCall",
    "Skill",
    "ErrorResponse",
    "UHorseError",
    "AuthenticationError",
    "NotFoundError",
    "ValidationError",
    "RateLimitError",
]

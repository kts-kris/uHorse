"""
uHorse SDK Types
"""

from datetime import datetime
from typing import Any, Dict, List, Optional
from pydantic import BaseModel, Field


class Agent(BaseModel):
    """Agent model."""

    id: str
    name: str
    description: Optional[str] = None
    channel: Optional[str] = None
    status: str = "active"
    created_at: Optional[datetime] = None
    updated_at: Optional[datetime] = None

    class Config:
        json_schema_extra = {
            "example": {
                "id": "agent-123",
                "name": "Customer Service Bot",
                "description": "Handles customer inquiries",
                "channel": "telegram",
                "status": "active",
            }
        }


class Session(BaseModel):
    """Session model."""

    id: str
    agent_id: str
    channel: str
    user_id: Optional[str] = None
    status: str = "active"
    created_at: Optional[datetime] = None
    updated_at: Optional[datetime] = None
    metadata: Dict[str, Any] = Field(default_factory=dict)

    class Config:
        json_schema_extra = {
            "example": {
                "id": "session-456",
                "agent_id": "agent-123",
                "channel": "telegram",
                "user_id": "user-789",
                "status": "active",
            }
        }


class Message(BaseModel):
    """Message model."""

    id: str
    session_id: str
    role: str  # "user", "assistant", "tool"
    content: str
    tool_calls: Optional[List["ToolCall"]] = None
    created_at: Optional[datetime] = None
    metadata: Dict[str, Any] = Field(default_factory=dict)

    class Config:
        json_schema_extra = {
            "example": {
                "id": "msg-789",
                "session_id": "session-456",
                "role": "assistant",
                "content": "Hello! How can I help you?",
            }
        }


class ToolCall(BaseModel):
    """Tool call model."""

    id: str
    name: str
    parameters: Dict[str, Any] = Field(default_factory=dict)
    result: Optional[Any] = None
    status: str = "pending"  # pending, success, error
    error: Optional[str] = None

    class Config:
        json_schema_extra = {
            "example": {
                "id": "call-123",
                "name": "search",
                "parameters": {"query": "weather"},
                "status": "success",
                "result": {"temperature": 25},
            }
        }


class Skill(BaseModel):
    """Skill model."""

    name: str
    description: Optional[str] = None
    version: Optional[str] = None
    parameters: Dict[str, Any] = Field(default_factory=dict)
    enabled: bool = True

    class Config:
        json_schema_extra = {
            "example": {
                "name": "calculator",
                "description": "Perform mathematical calculations",
                "parameters": {
                    "expression": {"type": "string", "required": True}
                },
            }
        }


class ErrorResponse(BaseModel):
    """Error response model."""

    error: str
    message: str
    details: Optional[Dict[str, Any]] = None

    class Config:
        json_schema_extra = {
            "example": {
                "error": "validation_error",
                "message": "Invalid input",
                "details": {"field": "name", "reason": "required"},
            }
        }


# Update forward references
Message.model_rebuild()

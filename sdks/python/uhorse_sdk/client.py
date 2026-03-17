"""
uHorse API Client
"""

import json
from typing import Any, Dict, List, Optional, AsyncIterator
import httpx
import websockets

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


class Client:
    """
    uHorse API Client

    Example:
        ```python
        from uhorse_sdk import Client

        async with Client("http://localhost:8080") as client:
            # List agents
            agents = await client.agents.list()

            # Send message
            response = await client.chat.send(
                session_id="session-123",
                message="Hello!"
            )
        ```
    """

    def __init__(
        self,
        base_url: str,
        api_key: Optional[str] = None,
        timeout: float = 30.0,
    ):
        """
        Initialize the client.

        Args:
            base_url: uHorse server URL (e.g., "http://localhost:8080")
            api_key: Optional API key for authentication
            timeout: Request timeout in seconds
        """
        self.base_url = base_url.rstrip("/")
        self.api_key = api_key
        self.timeout = timeout

        self._http: Optional[httpx.AsyncClient] = None

        # Sub-clients
        self.agents = AgentsClient(self)
        self.sessions = SessionsClient(self)
        self.chat = ChatClient(self)
        self.skills = SkillsClient(self)

    async def __aenter__(self) -> "Client":
        await self.connect()
        return self

    async def __aexit__(self, exc_type, exc_val, exc_tb):
        await self.close()

    async def connect(self):
        """Initialize the HTTP client."""
        if self._http is None:
            headers = {}
            if self.api_key:
                headers["Authorization"] = f"Bearer {self.api_key}"

            self._http = httpx.AsyncClient(
                base_url=self.base_url,
                headers=headers,
                timeout=self.timeout,
            )

    async def close(self):
        """Close the HTTP client."""
        if self._http:
            await self._http.aclose()
            self._http = None

    async def request(
        self,
        method: str,
        path: str,
        **kwargs,
    ) -> Any:
        """Make an HTTP request."""
        if self._http is None:
            await self.connect()

        response = await self._http.request(method, path, **kwargs)

        if response.status_code >= 400:
            await self._handle_error(response)

        return response.json()

    async def _handle_error(self, response: httpx.Response):
        """Handle HTTP error responses."""
        try:
            error_data = response.json()
            error = ErrorResponse(**error_data)
        except Exception:
            error = ErrorResponse(
                error="unknown",
                message=response.text or "Unknown error",
            )

        if response.status_code == 401:
            raise AuthenticationError(error.message)
        elif response.status_code == 404:
            raise NotFoundError(error.message)
        elif response.status_code == 422:
            raise ValidationError(error.message)
        elif response.status_code == 429:
            raise RateLimitError(error.message)
        else:
            raise UHorseError(error.message)


class AgentsClient:
    """Client for agent operations."""

    def __init__(self, client: Client):
        self._client = client

    async def list(self) -> List[Agent]:
        """List all agents."""
        data = await self._client.request("GET", "/api/v1/agents")
        return [Agent(**a) for a in data.get("agents", [])]

    async def get(self, agent_id: str) -> Agent:
        """Get agent by ID."""
        data = await self._client.request("GET", f"/api/v1/agents/{agent_id}")
        return Agent(**data)

    async def create(
        self,
        name: str,
        description: Optional[str] = None,
        config: Optional[Dict[str, Any]] = None,
    ) -> Agent:
        """Create a new agent."""
        payload = {"name": name}
        if description:
            payload["description"] = description
        if config:
            payload["config"] = config

        data = await self._client.request("POST", "/api/v1/agents", json=payload)
        return Agent(**data)

    async def delete(self, agent_id: str) -> None:
        """Delete an agent."""
        await self._client.request("DELETE", f"/api/v1/agents/{agent_id}")


class SessionsClient:
    """Client for session operations."""

    def __init__(self, client: Client):
        self._client = client

    async def list(
        self,
        agent_id: Optional[str] = None,
        channel: Optional[str] = None,
    ) -> List[Session]:
        """List sessions with optional filters."""
        params = {}
        if agent_id:
            params["agent_id"] = agent_id
        if channel:
            params["channel"] = channel

        data = await self._client.request("GET", "/api/v1/sessions", params=params)
        return [Session(**s) for s in data.get("sessions", [])]

    async def get(self, session_id: str) -> Session:
        """Get session by ID."""
        data = await self._client.request("GET", f"/api/v1/sessions/{session_id}")
        return Session(**data)

    async def create(
        self,
        agent_id: str,
        channel: str,
        user_id: Optional[str] = None,
    ) -> Session:
        """Create a new session."""
        payload = {
            "agent_id": agent_id,
            "channel": channel,
        }
        if user_id:
            payload["user_id"] = user_id

        data = await self._client.request("POST", "/api/v1/sessions", json=payload)
        return Session(**data)

    async def delete(self, session_id: str) -> None:
        """Delete a session."""
        await self._client.request("DELETE", f"/api/v1/sessions/{session_id}")


class ChatClient:
    """Client for chat operations."""

    def __init__(self, client: Client):
        self._client = client

    async def send(
        self,
        session_id: str,
        message: str,
        metadata: Optional[Dict[str, Any]] = None,
    ) -> Message:
        """Send a message to a session."""
        payload = {
            "session_id": session_id,
            "content": message,
        }
        if metadata:
            payload["metadata"] = metadata

        data = await self._client.request("POST", "/api/v1/chat/messages", json=payload)
        return Message(**data)

    async def history(
        self,
        session_id: str,
        limit: int = 50,
    ) -> List[Message]:
        """Get message history for a session."""
        data = await self._client.request(
            "GET",
            f"/api/v1/sessions/{session_id}/messages",
            params={"limit": limit},
        )
        return [Message(**m) for m in data.get("messages", [])]

    async def stream(
        self,
        session_id: str,
        message: str,
    ) -> AsyncIterator[str]:
        """Stream a chat response."""
        ws_url = self._client.base_url.replace("http", "ws") + "/api/v1/chat/stream"

        async with websockets.connect(ws_url) as ws:
            # Send message
            await ws.send(json.dumps({
                "session_id": session_id,
                "content": message,
            }))

            # Receive chunks
            async for chunk in ws:
                data = json.loads(chunk)
                if data.get("type") == "chunk":
                    yield data.get("content", "")
                elif data.get("type") == "done":
                    break
                elif data.get("type") == "error":
                    raise UHorseError(data.get("message", "Stream error"))


class SkillsClient:
    """Client for skill operations."""

    def __init__(self, client: Client):
        self._client = client

    async def list(self) -> List[Skill]:
        """List all skills."""
        data = await self._client.request("GET", "/api/v1/skills")
        return [Skill(**s) for s in data.get("skills", [])]

    async def get(self, skill_name: str) -> Skill:
        """Get skill by name."""
        data = await self._client.request("GET", f"/api/v1/skills/{skill_name}")
        return Skill(**data)

    async def execute(
        self,
        skill_name: str,
        parameters: Dict[str, Any],
    ) -> Any:
        """Execute a skill."""
        data = await self._client.request(
            "POST",
            f"/api/v1/skills/{skill_name}/execute",
            json={"parameters": parameters},
        )
        return data.get("result")

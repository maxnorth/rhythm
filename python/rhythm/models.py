"""Data models"""

import json
from datetime import datetime
from enum import Enum
from typing import Any, Optional

from pydantic import BaseModel, Field


class ExecutionType(str, Enum):
    """Type of execution"""

    TASK = "task"
    WORKFLOW = "workflow"


class ExecutionStatus(str, Enum):
    """Status of an execution"""

    PENDING = "pending"
    RUNNING = "running"
    SUSPENDED = "suspended"
    COMPLETED = "completed"
    FAILED = "failed"


class Execution(BaseModel):
    """An execution (task or workflow)"""

    id: str
    type: ExecutionType
    target_name: str
    queue: str
    status: ExecutionStatus

    inputs: dict[str, Any] = Field(default_factory=dict)
    output: Optional[Any] = None

    attempt: int = 0

    parent_workflow_id: Optional[str] = None

    created_at: datetime
    completed_at: Optional[datetime] = None

    @classmethod
    def from_record(cls, record) -> "Execution":
        """Create from database record"""
        data = dict(record)
        # Parse JSONB fields
        for field in ["inputs", "output"]:
            if field in data and data[field] is not None:
                if isinstance(data[field], str):
                    data[field] = json.loads(data[field])
        return cls(**data)

    @classmethod
    def from_dict(cls, data: dict) -> "Execution":
        """Create from dictionary (e.g., from Rust)"""
        # Rust returns exec_type as "type", rename it
        if "exec_type" in data:
            data["type"] = data.pop("exec_type")
        return cls(**data)


class DelegatedAction(BaseModel):
    """Action delegated from Rust cooperative worker loop to host

    Action types:
    - execute_task: Execute a task (has execution_id, target_name, inputs)
    - continue: Continue immediately, check for more work
    - wait: Wait for duration_ms before checking for more work
    - shutdown: Shutdown requested, worker should exit gracefully
    """

    type: str

    # Fields for execute_task action
    execution_id: Optional[str] = None
    target_name: Optional[str] = None
    inputs: Optional[dict[str, Any]] = None

    # Fields for wait action
    duration_ms: Optional[int] = None

    @classmethod
    def from_dict(cls, data: dict) -> "DelegatedAction":
        """Create from dictionary (from Rust)"""
        return cls(**data)

"""Data models"""

from datetime import datetime
from enum import Enum
from typing import Any, Optional
from pydantic import BaseModel, Field
import json


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
    function_name: str
    queue: str
    status: ExecutionStatus

    inputs: dict[str, Any] = Field(default_factory=dict)
    output: Optional[Any] = None

    attempt: int = 0
    max_retries: int = 3

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

"""Utility functions"""

import uuid
from typing import Any
import json


def generate_id(prefix: str = "") -> str:
    """Generate a unique ID"""
    unique = str(uuid.uuid4())
    return f"{prefix}_{unique}" if prefix else unique


def serialize_args(*args, **kwargs) -> tuple[str, str]:
    """Serialize args and kwargs to JSON strings"""
    return json.dumps(args), json.dumps(kwargs)


def calculate_retry_delay(attempt: int, base: float = 2.0, max_delay: float = 60.0) -> float:
    """Calculate exponential backoff delay"""
    delay = base * (2**attempt)
    return min(delay, max_delay)


def is_json_serializable(obj: Any) -> bool:
    """Check if an object is JSON serializable"""
    try:
        json.dumps(obj)
        return True
    except (TypeError, ValueError):
        return False

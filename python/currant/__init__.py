"""
Currant - A lightweight durable execution framework using only Postgres
"""

from currant.decorators import task, workflow
from currant.client import send_signal
from currant.context import get_version, is_replaying, wait_for_signal

__all__ = [
    "task",
    "workflow",
    "send_signal",
    "get_version",
    "is_replaying",
    "wait_for_signal",
]

__version__ = "0.1.0"

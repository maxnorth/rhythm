"""
Currant - A lightweight durable execution framework using only Postgres
"""

from currant.decorators import task, workflow
from currant.client import send_signal, start_workflow
from currant.context import get_version, is_replaying, wait_for_signal
from currant.init import init

__all__ = [
    "init",
    "task",
    "workflow",
    "send_signal",
    "start_workflow",
    "get_version",
    "is_replaying",
    "wait_for_signal",
]

__version__ = "0.1.0"

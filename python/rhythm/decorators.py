"""Decorators for defining tasks"""

from typing import Callable, Optional

from rhythm.registry import register_function
from rhythm.client import queue_execution


def task(fn: Optional[Callable] = None, *, queue: str = "default"):
    """
    Decorator for defining a task (standalone task).

    Args:
        queue: The queue name to execute in (defaults to "default")

    Example:
        # Simple usage with default queue
        @task
        def send_email(to: str, subject: str):
            email_client.send(to, subject)

        # Or specify a queue
        @task(queue="emails")
        def send_notification(user_id: str, message: str):
            pass

        # Direct call
        send_email("user@example.com", "Hello")

        # Queue for async execution
        send_email.queue(to="user@example.com", subject="Hello")
    """

    def decorator(func: Callable) -> Callable:
        # Register the function in the registry
        register_function(func.__name__, func)

        # Add a queue method to the function
        def queue_fn(**inputs) -> str:
            """Enqueue this task for execution"""
            return queue_execution(
                exec_type="task",
                function_name=func.__name__,
                inputs=inputs,
                queue=queue,
            )

        func.queue = queue_fn
        return func

    # Support both @task and @task() and @task(queue="name")
    if fn is None:
        # Called with arguments: @task() or @task(queue="name")
        return decorator
    else:
        # Called without arguments: @task
        return decorator(fn)

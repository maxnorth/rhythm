"""rhythm.task - Task decorator"""

from typing import Callable, Optional

from rhythm.client import queue_execution
from rhythm.registry import register_function


def task(fn: Optional[Callable] = None, *, queue: str = "default"):
    """Mark a function as a Rhythm task that can be queued for async execution.

    Decorated functions can be called directly (synchronous) or queued for
    async execution via the added `.queue()` method.

    Args:
        queue: The queue name to execute in (defaults to "default")

    Returns:
        The decorated function with an added `.queue()` method

    Example:
        @task
        def send_email(to: str, subject: str):
            email_client.send(to, subject)

        # Direct call (synchronous)
        send_email("user@example.com", "Hello")

        # Queue for async execution
        execution_id = send_email.queue(to="user@example.com", subject="Hello")

    Meta:
        section: Tasks
        kind: decorator
    """

    def decorator(func: Callable) -> Callable:
        # Register the function in the registry
        register_function(func.__name__, func)

        # Add a queue method to the function
        def queue_fn(**inputs) -> str:
            """Enqueue this task for execution"""
            return queue_execution(
                exec_type="task",
                target_name=func.__name__,
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

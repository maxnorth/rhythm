"""rhythm.task - Task decorator"""

from typing import Callable, Optional

from rhythm.client import queue_execution
from rhythm.registry import register_function


def task(fn: Optional[Callable] = None, *, name: Optional[str] = None, queue: str = "default"):
    """Mark a function as a Rhythm task that can be queued for async execution.

    Decorated functions can be called directly (synchronous) or queued for
    async execution via the added `.queue()` method.

    Args:
        name: Custom task name (defaults to function name). Useful for kebab-case names.
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

        # With custom name
        @task(name="send-notification")
        def send_notification(user_id: str, message: str):
            ...

    Meta:
        section: Tasks
        kind: decorator
    """

    def decorator(func: Callable) -> Callable:
        task_name = name if name is not None else func.__name__

        # Register the function in the registry
        register_function(task_name, func)

        # Add a queue method to the function
        def queue_fn(**inputs) -> str:
            """Enqueue this task for execution"""
            return queue_execution(
                exec_type="task",
                target_name=task_name,
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

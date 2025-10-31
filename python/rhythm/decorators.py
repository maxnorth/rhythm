"""Decorators for defining tasks"""

from typing import Callable, Optional
import inspect

from rhythm.config import settings
from rhythm.registry import register_function
from rhythm.client import queue_execution


class ExecutableProxy:
    """Base proxy for executable functions (tasks)"""

    def __init__(
        self,
        fn: Callable,
        exec_type: str,
        queue: Optional[str] = None,
        retries: Optional[int] = None,
        timeout: Optional[int] = None,
        priority: int = 5,
        **extra_config,
    ):
        self.fn = fn
        self.exec_type = exec_type
        # Use just the function name without module prefix
        # This allows DSL workflows to reference tasks by simple name
        self.function_name = fn.__name__

        # Store configuration
        self.config = {
            "queue": queue,
            "retries": retries or settings.default_retries,
            "timeout": timeout,
            "priority": priority,
            **extra_config,
        }

        # Register the function
        register_function(self.function_name, fn)

    def options(self, **opts) -> "ExecutableProxy":
        """Return a new proxy with modified options"""
        new_config = {**self.config, **opts}
        return ExecutableProxy(
            fn=self.fn,
            exec_type=self.exec_type,
            **new_config,
        )

    async def queue(self, *args, **kwargs) -> str:
        """Enqueue this execution"""

        return await queue_execution(
            exec_type=self.exec_type,
            function_name=self.function_name,
            args=args,
            kwargs=kwargs,
            queue=self.config["queue"],
            priority=self.config["priority"],
            max_retries=self.config["retries"],
            timeout_seconds=self.config["timeout"],
        )

    def __call__(self, *args, **kwargs):
        """Direct call - execute the function directly"""
        return self.fn(*args, **kwargs)


class TaskProxy(ExecutableProxy):
    """Proxy for task functions"""

    def __init__(self, fn: Callable, queue: str, **config):
        if queue is None:
            raise ValueError("@task decorator requires a 'queue' parameter")

        if "timeout" not in config or config["timeout"] is None:
            config["timeout"] = settings.default_timeout
        super().__init__(fn, exec_type="task", queue=queue, **config)


def task(
    queue: str,
    retries: int = None,
    timeout: int = None,
    priority: int = 5,
):
    """
    Decorator for defining a task (standalone async task).

    Args:
        queue: The queue name to execute in
        retries: Number of retry attempts (default: 3)
        timeout: Timeout in seconds (default: 300)
        priority: Priority 0-10, higher = more urgent (default: 5)

    Example:
        @task(queue="emails", retries=3)
        async def send_email(to: str, subject: str):
            await email_client.send(to, subject)
    """

    def decorator(fn: Callable) -> TaskProxy:
        if not inspect.iscoroutinefunction(fn):
            raise TypeError(f"@task decorated function must be async: {fn.__name__}")

        return TaskProxy(
            fn=fn,
            queue=queue,
            retries=retries,
            timeout=timeout,
            priority=priority,
        )

    return decorator

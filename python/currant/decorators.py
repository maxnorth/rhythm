"""Decorators for defining jobs, activities, and workflows"""

from typing import Any, Callable, Optional
from functools import wraps
import inspect

from currant.config import settings
from currant.registry import register_function
from currant.client import queue_execution


class ExecutableProxy:
    """Base proxy for executable functions (jobs, activities, workflows)"""

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
        self.function_name = f"{fn.__module__}.{fn.__qualname__}"

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

    async def run(self, *args, **kwargs) -> Any:
        """
        Run this execution within a workflow context.
        This will checkpoint and suspend the workflow.
        """
        from currant.context import get_current_workflow_context

        ctx = get_current_workflow_context()
        if ctx is None:
            raise RuntimeError(
                f"{self.exec_type}.run() can only be called from within a workflow. "
                f"Use .queue() to run standalone."
            )

        return await ctx.execute_activity(self, args, kwargs)

    def __call__(self, *args, **kwargs):
        """Direct call - only allowed outside workflow context for testing"""
        return self.fn(*args, **kwargs)


class JobProxy(ExecutableProxy):
    """Proxy for job functions"""

    def __init__(self, fn: Callable, queue: str, **config):
        if queue is None:
            raise ValueError("@job decorator requires a 'queue' parameter")

        if "timeout" not in config or config["timeout"] is None:
            config["timeout"] = settings.default_timeout
        super().__init__(fn, exec_type="job", queue=queue, **config)


class ActivityProxy(ExecutableProxy):
    """Proxy for activity functions"""

    def __init__(self, fn: Callable, **config):
        if "timeout" not in config or config["timeout"] is None:
            config["timeout"] = settings.default_timeout
        super().__init__(fn, exec_type="activity", **config)


class WorkflowProxy(ExecutableProxy):
    """Proxy for workflow functions"""

    def __init__(self, fn: Callable, queue: str, version: int = 1, **config):
        if queue is None:
            raise ValueError("@workflow decorator requires a 'queue' parameter")

        if "timeout" not in config or config["timeout"] is None:
            config["timeout"] = settings.default_workflow_timeout

        super().__init__(
            fn,
            exec_type="workflow",
            queue=queue,
            version=version,
            **config,
        )
        self.version = version


def job(
    queue: str,
    retries: int = None,
    timeout: int = None,
    priority: int = 5,
):
    """
    Decorator for defining a job (standalone async task).

    Args:
        queue: The queue name to execute in
        retries: Number of retry attempts (default: 3)
        timeout: Timeout in seconds (default: 300)
        priority: Priority 0-10, higher = more urgent (default: 5)

    Example:
        @job(queue="emails", retries=3)
        async def send_email(to: str, subject: str):
            await email_client.send(to, subject)
    """

    def decorator(fn: Callable) -> JobProxy:
        if not inspect.iscoroutinefunction(fn):
            raise TypeError(f"@job decorated function must be async: {fn.__name__}")

        return JobProxy(
            fn=fn,
            queue=queue,
            retries=retries,
            timeout=timeout,
            priority=priority,
        )

    return decorator


def activity(
    retries: int = None,
    timeout: int = None,
    priority: int = 5,
):
    """
    Decorator for defining an activity (workflow step).

    Activities are called from currant via .run() and inherit the workflow's queue.

    Args:
        retries: Number of retry attempts (default: 3)
        timeout: Timeout in seconds (default: 300)
        priority: Priority 0-10, higher = more urgent (default: 5)

    Example:
        @activity(retries=3)
        async def charge_card(amount: int, card_token: str):
            return await payment_api.charge(amount, card_token)
    """

    def decorator(fn: Callable) -> ActivityProxy:
        if not inspect.iscoroutinefunction(fn):
            raise TypeError(f"@activity decorated function must be async: {fn.__name__}")

        return ActivityProxy(
            fn=fn,
            retries=retries,
            timeout=timeout,
            priority=priority,
        )

    return decorator


def workflow(
    queue: str,
    version: int = 1,
    timeout: int = None,
    priority: int = 5,
):
    """
    Decorator for defining a workflow (multi-step orchestration).

    Workflows must be deterministic and use replay for recovery.

    Args:
        queue: The queue name to execute in
        version: Version number for workflow evolution (default: 1)
        timeout: Timeout in seconds (default: 3600)
        priority: Priority 0-10, higher = more urgent (default: 5)

    Example:
        @workflow(queue="orders", version=1)
        async def process_order(order_id: str):
            result = await charge_card.run(amount, token)
            await send_receipt.run(email, amount)
            return result
    """

    def decorator(fn: Callable) -> WorkflowProxy:
        if not inspect.iscoroutinefunction(fn):
            raise TypeError(f"@workflow decorated function must be async: {fn.__name__}")

        return WorkflowProxy(
            fn=fn,
            queue=queue,
            version=version,
            timeout=timeout,
            priority=priority,
        )

    return decorator

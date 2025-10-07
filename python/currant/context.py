"""Workflow execution context and replay mechanism"""

from contextvars import ContextVar
from typing import Any, Optional
import logging

from currant.utils import generate_id

logger = logging.getLogger(__name__)

# Context variable for the current workflow execution
_current_workflow_context: ContextVar[Optional["WorkflowExecutionContext"]] = ContextVar(
    "workflow_context", default=None
)


class WorkflowSuspendException(Exception):
    """
    Raised when a workflow needs to suspend execution.
    This is caught by the worker to checkpoint and suspend the workflow.
    """

    def __init__(self, commands: list[dict]):
        self.commands = commands
        super().__init__("Workflow suspended")


class WorkflowExecutionContext:
    """
    Context for a workflow execution.
    Manages replay state and activity execution.
    """

    def __init__(self, execution_id: str, checkpoint: Optional[dict] = None):
        self.execution_id = execution_id
        self.checkpoint = checkpoint or {}
        self.history = self.checkpoint.get("history", [])
        self.current_step_index = 0
        self.is_replaying = len(self.history) > 0
        self.new_commands = []
        self.pending_signals = {}

        logger.debug(
            f"WorkflowExecutionContext created for {execution_id}, "
            f"history length: {len(self.history)}, replaying: {self.is_replaying}"
        )

    def get_next_history_event(self) -> Optional[dict]:
        """Get the next event from history during replay"""
        if self.current_step_index < len(self.history):
            event = self.history[self.current_step_index]
            self.current_step_index += 1
            logger.debug(f"Replaying step {self.current_step_index}: {event.get('type')}")
            return event
        return None

    async def execute_activity(self, activity_proxy, args: tuple, kwargs: dict) -> Any:
        """
        Execute an activity within the workflow.
        Either returns cached result (replay) or suspends to execute activity.
        """
        # Check if we're replaying this step
        history_event = self.get_next_history_event()

        if history_event:
            # REPLAY MODE: return cached result
            assert history_event["type"] == "activity", "History mismatch: expected activity"
            assert history_event["name"] == activity_proxy.function_name, (
                f"History mismatch: expected {activity_proxy.function_name}, got {history_event['name']}"
            )

            logger.debug(f"Replaying activity {activity_proxy.function_name}")
            return history_event["result"]
        else:
            # NEW STEP: we've finished replaying, now executing new steps
            self.is_replaying = False
            activity_execution_id = generate_id("act")

            logger.debug(f"Suspending workflow to execute activity {activity_proxy.function_name}")

            # Record command to execute this activity
            self.new_commands.append(
                {
                    "type": "activity",
                    "activity_execution_id": activity_execution_id,
                    "name": activity_proxy.function_name,
                    "args": list(args),
                    "kwargs": kwargs,
                    "config": activity_proxy.config,
                }
            )

            # Suspend workflow execution
            raise WorkflowSuspendException(self.new_commands)

    async def wait_for_signal(
        self, signal_name: str, timeout: Optional[float] = None
    ) -> dict[str, Any]:
        """
        Wait for a signal to be sent to this workflow.
        This will suspend the workflow until the signal arrives.
        """
        # Check if we're replaying this step
        history_event = self.get_next_history_event()

        if history_event:
            # REPLAY MODE: return cached signal
            assert history_event["type"] == "signal", "History mismatch: expected signal"
            assert history_event["signal_name"] == signal_name, (
                f"History mismatch: expected signal {signal_name}, got {history_event['signal_name']}"
            )

            logger.debug(f"Replaying signal {signal_name}")
            return history_event["payload"]
        else:
            # NEW STEP: we've finished replaying, now executing new steps
            self.is_replaying = False
            logger.debug(f"Suspending workflow to wait for signal {signal_name}")

            # Record command to wait for signal
            self.new_commands.append(
                {
                    "type": "wait_signal",
                    "signal_name": signal_name,
                    "timeout": timeout,
                }
            )

            # Suspend workflow execution
            raise WorkflowSuspendException(self.new_commands)

    def get_version(self, change_id: str, min_version: int, max_version: int) -> int:
        """
        Get the version for a particular change point in the workflow.
        This allows workflows to evolve while maintaining compatibility.
        """
        # Check if we're replaying this step
        history_event = self.get_next_history_event()

        if history_event:
            # REPLAY MODE: return cached version
            assert history_event["type"] == "version", "History mismatch: expected version"
            assert history_event["change_id"] == change_id, (
                f"History mismatch: expected change_id {change_id}, got {history_event['change_id']}"
            )

            version = history_event["version"]
            logger.debug(f"Replaying version check {change_id} = {version}")
            return version
        else:
            # NEW STEP: record the max version (current version)
            logger.debug(f"Recording version check {change_id} = {max_version}")

            # Add to history immediately (version checks don't suspend)
            self.history.append(
                {
                    "type": "version",
                    "change_id": change_id,
                    "version": max_version,
                }
            )
            self.current_step_index += 1

            return max_version


def get_current_workflow_context() -> Optional[WorkflowExecutionContext]:
    """Get the current workflow execution context"""
    return _current_workflow_context.get()


def set_current_workflow_context(ctx: Optional[WorkflowExecutionContext]):
    """Set the current workflow execution context"""
    _current_workflow_context.set(ctx)


def clear_current_workflow_context():
    """Clear the current workflow execution context"""
    _current_workflow_context.set(None)


# Public API functions for use within workflows


async def wait_for_signal(signal_name: str, timeout: Optional[float] = None) -> dict[str, Any]:
    """
    Wait for a signal to be sent to this workflow.

    Args:
        signal_name: Name of the signal to wait for
        timeout: Optional timeout in seconds

    Returns:
        The signal payload

    Example:
        @workflow(queue="approvals", version=1)
        async def approval_workflow(doc_id: str):
            approval = await wait_for_signal("approved", timeout=86400)
            if approval["approved"]:
                await process_document.run(doc_id)
    """
    ctx = get_current_workflow_context()
    if ctx is None:
        raise RuntimeError("wait_for_signal() can only be called from within a workflow")

    return await ctx.wait_for_signal(signal_name, timeout)


def get_version(change_id: str, min_version: int, max_version: int) -> int:
    """
    Get the version for a particular change point in the workflow.
    Allows workflows to evolve while maintaining backward compatibility.

    Args:
        change_id: Unique identifier for this change point
        min_version: Minimum supported version
        max_version: Current/maximum version

    Returns:
        The version that was active when this workflow was started

    Example:
        @workflow(queue="orders", version=2)
        async def process_order(order_id: str):
            result = await charge_card.run(amount, token)

            # New step added in version 2
            if get_version("send_sms", 1, 2) >= 2:
                await send_sms.run(phone, "Order confirmed")

            await send_receipt.run(email, amount)
    """
    ctx = get_current_workflow_context()
    if ctx is None:
        raise RuntimeError("get_version() can only be called from within a workflow")

    return ctx.get_version(change_id, min_version, max_version)


def is_replaying() -> bool:
    """
    Check if the workflow is currently replaying from history.

    Returns True during replay, False when executing new steps.
    Useful for conditional logging or other non-deterministic operations
    that should only run once.

    Returns:
        True if replaying, False otherwise

    Example:
        @workflow(queue="orders", version=1)
        async def process_order(order_id: str):
            if not is_replaying():
                print(f"[WORKFLOW] Starting order processing for {order_id}")

            result = await validate_order.run(order_id)

            if not is_replaying():
                print(f"[WORKFLOW] Validation completed: {result}")
    """
    ctx = get_current_workflow_context()
    if ctx is None:
        raise RuntimeError("is_replaying() can only be called from within a workflow")

    return ctx.is_replaying

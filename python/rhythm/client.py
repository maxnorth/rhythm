"""Client API for enqueuing work and sending signals"""

import logging
import time
from typing import Any, Optional

from rhythm.core import RhythmCore
from rhythm.models import Execution, ExecutionStatus

logger = logging.getLogger(__name__)


def queue_task(
    name: str,
    inputs: dict,
    queue: str = "default",
) -> str:
    """Queue a task for execution.

    Args:
        name: Task function name
        inputs: Input parameters as a dictionary
        queue: Queue name (default: "default")

    Returns:
        Execution ID

    Meta:
        section: Client
    """
    execution_id = RhythmCore.create_execution(
        exec_type="task",
        target_name=name,
        queue=queue,
        inputs=inputs,
        parent_workflow_id=None,
    )

    logger.info(f"Enqueued task {execution_id}: {name} on queue {queue}")
    return execution_id


def queue_workflow(
    name: str,
    inputs: dict,
    queue: str = "default",
) -> str:
    """Queue a workflow for execution.

    Args:
        name: Workflow name
        inputs: Input parameters as a dictionary
        queue: Queue name (default: "default")

    Returns:
        Execution ID

    Meta:
        section: Client
    """
    execution_id = RhythmCore.create_execution(
        exec_type="workflow",
        target_name=name,
        queue=queue,
        inputs=inputs,
        parent_workflow_id=None,
    )

    logger.info(f"Enqueued workflow {execution_id}: {name} on queue {queue}")
    return execution_id


def queue_execution(
    exec_type: str,
    target_name: str,
    inputs: dict,
    queue: str,
    parent_workflow_id: Optional[str] = None,
) -> str:
    """Enqueue an execution (task or workflow).

    Note: Prefer using queue_task() or queue_workflow() for better type safety.

    Args:
        exec_type: Type of execution ('task', 'workflow')
        target_name: Target name (task or workflow name)
        inputs: Input parameters as a dictionary
        queue: Queue name
        parent_workflow_id: Parent workflow ID (for workflow tasks)

    Returns:
        Execution ID

    Meta:
        section: Client
    """
    execution_id = RhythmCore.create_execution(
        exec_type=exec_type,
        target_name=target_name,
        queue=queue,
        inputs=inputs,
        parent_workflow_id=parent_workflow_id,
    )

    logger.info(f"Enqueued {exec_type} {execution_id}: {target_name} on queue {queue}")
    return execution_id


def get_execution(execution_id: str) -> Optional[Execution]:
    """Get an execution by ID.

    Args:
        execution_id: The execution ID

    Returns:
        Execution object or None if not found

    Meta:
        section: Client
    """
    return RhythmCore.get_execution(execution_id)


def cancel_execution(execution_id: str) -> bool:
    """Cancel a pending or suspended execution.

    Args:
        execution_id: The execution ID

    Returns:
        True if cancelled, False if not found or already completed/running

    Meta:
        section: Client
    """
    try:
        RhythmCore.fail_execution(
            execution_id,
            {"message": "Execution cancelled", "type": "CancellationError"},
            retry=False,
        )
        logger.info(f"Execution {execution_id} cancelled")
        return True
    except Exception as e:
        logger.warning(f"Could not cancel execution {execution_id}: {e}")
        return False


def start_workflow(workflow_name: str, inputs: dict[str, Any]) -> str:
    """Start a workflow execution.

    Args:
        workflow_name: Name of the workflow to execute (matches .flow filename)
        inputs: Input parameters for the workflow

    Returns:
        Workflow execution ID

    Example:
        workflow_id = rhythm.start_workflow(
            "processOrder",
            inputs={"orderId": "order-123", "amount": 99.99}
        )

    Meta:
        section: Client
    """
    execution_id = RhythmCore.start_workflow(workflow_name, inputs)
    logger.info(f"Started workflow {workflow_name} with ID {execution_id}")
    return execution_id


def list_executions(
    queue: Optional[str] = None,
    status: Optional[str] = None,
    limit: int = 100,
    offset: int = 0,
) -> list[dict]:
    """List executions with optional filters.

    NOTE: This function is currently not implemented as it requires direct database access.
    Use the Rust bridge functions instead for execution management.

    Args:
        queue: Filter by queue name
        status: Filter by status
        limit: Maximum number of results
        offset: Offset for pagination

    Returns:
        List of execution dicts

    Meta:
        section: Client
    """
    raise NotImplementedError(
        "list_executions is not yet implemented. Use Rust bridge functions for execution management."
    )


def schedule_task(
    name: str,
    inputs: dict,
    run_at: str,
    queue: str = "default",
) -> str:
    """Schedule a task for execution at a future time.

    Creates the execution immediately in Pending status, then schedules
    it to be enqueued at the specified time.

    Args:
        name: Task function name
        inputs: Input parameters as a dictionary
        run_at: ISO 8601 datetime string (e.g., "2024-01-15T10:30:00")
        queue: Queue name (default: "default")

    Returns:
        Execution ID

    Meta:
        section: Client
    """
    execution_id = RhythmCore.schedule_task(
        task_name=name,
        inputs=inputs,
        run_at=run_at,
        queue=queue,
    )

    logger.info(f"Scheduled task {execution_id}: {name} to run at {run_at}")
    return execution_id


def schedule_workflow(
    name: str,
    inputs: dict,
    run_at: str,
    queue: str = "default",
) -> str:
    """Schedule a workflow for execution at a future time.

    Creates the execution immediately in Pending status, then schedules
    it to be enqueued at the specified time.

    Args:
        name: Workflow name
        inputs: Input parameters as a dictionary
        run_at: ISO 8601 datetime string (e.g., "2024-01-15T10:30:00")
        queue: Queue name (default: "default")

    Returns:
        Execution ID

    Meta:
        section: Client
    """
    execution_id = RhythmCore.schedule_workflow(
        workflow_name=name,
        inputs=inputs,
        run_at=run_at,
        queue=queue,
    )

    logger.info(f"Scheduled workflow {execution_id}: {name} to run at {run_at}")
    return execution_id


def send_signal(
    workflow_id: str,
    signal_name: str,
    payload: Any,
    queue: str = "default",
) -> None:
    """Send a signal to a workflow.

    The workflow will be enqueued for processing and will pick up the
    signal on its next resumption.

    Args:
        workflow_id: ID of the workflow to send the signal to
        signal_name: Name of the signal channel
        payload: Signal payload (any JSON-serializable value)
        queue: Queue name (default: "default")

    Example:
        rhythm.send_signal(
            workflow_id="abc-123",
            signal_name="approval",
            payload={"approved": True, "reviewer": "alice"}
        )

    Meta:
        section: Client
    """
    RhythmCore.send_signal(
        workflow_id=workflow_id,
        signal_name=signal_name,
        payload=payload,
        queue=queue,
    )
    logger.info(f"Sent signal '{signal_name}' to workflow {workflow_id}")


def wait_for_execution(
    execution_id: str,
    timeout: float = 60.0,
    poll_interval: float = 0.5,
) -> Execution:
    """Wait for an execution to reach a terminal state and return it.

    Polls the execution status until it reaches "completed" or "failed" status.

    Args:
        execution_id: The execution ID to wait for
        timeout: Maximum time to wait in seconds (default: 60)
        poll_interval: How often to poll in seconds (default: 0.5)

    Returns:
        Execution object with full execution details

    Raises:
        TimeoutError: If execution doesn't reach terminal state within timeout
        RuntimeError: If execution not found

    Meta:
        section: Client
    """
    start_time = time.time()

    while True:
        execution = get_execution(execution_id)

        if execution is None:
            raise RuntimeError(f"Execution {execution_id} not found")

        # Check if reached terminal state
        if execution.status in (ExecutionStatus.COMPLETED, ExecutionStatus.FAILED):
            return execution

        # Check timeout
        if time.time() - start_time > timeout:
            raise TimeoutError(
                f"Execution {execution_id} did not complete within {timeout}s "
                f"(current status: {execution.status})"
            )

        time.sleep(poll_interval)

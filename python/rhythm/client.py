"""Client API for enqueuing work and sending signals"""

import logging
from typing import Any, Optional

from rhythm.rust_bridge import RustBridge

logger = logging.getLogger(__name__)


async def queue_execution(
    exec_type: str,
    function_name: str,
    args: tuple,
    kwargs: dict,
    queue: str,
    priority: int = 5,
    max_retries: int = 3,
    timeout_seconds: Optional[int] = None,
    parent_workflow_id: Optional[str] = None,
) -> str:
    """
    Enqueue an execution (task or workflow).

    Args:
        exec_type: Type of execution ('task', 'workflow')
        function_name: Fully qualified function name
        args: Positional arguments
        kwargs: Keyword arguments
        queue: Queue name
        priority: Priority (0-10, higher = more urgent)
        max_retries: Maximum retry attempts
        timeout_seconds: Timeout in seconds
        parent_workflow_id: Parent workflow ID (for workflow tasks)

    Returns:
        Execution ID
    """
    execution_id = RustBridge.create_execution(
        exec_type=exec_type,
        function_name=function_name,
        queue=queue,
        priority=priority,
        args=list(args),
        kwargs=kwargs,
        max_retries=max_retries,
        timeout_seconds=timeout_seconds,
        parent_workflow_id=parent_workflow_id,
    )

    logger.info(f"Enqueued {exec_type} {execution_id}: {function_name} on queue {queue}")
    return execution_id


async def get_execution_status(execution_id: str) -> Optional[dict]:
    """
    Get the status of an execution.

    Args:
        execution_id: The execution ID

    Returns:
        Execution status dict or None if not found
    """
    return RustBridge.get_execution(execution_id)


async def cancel_execution(execution_id: str) -> bool:
    """
    Cancel a pending or suspended execution.

    Args:
        execution_id: The execution ID

    Returns:
        True if cancelled, False if not found or already completed/running
    """
    try:
        RustBridge.fail_execution(
            execution_id,
            {"message": "Execution cancelled", "type": "CancellationError"},
            retry=False,
        )
        logger.info(f"Execution {execution_id} cancelled")
        return True
    except Exception as e:
        logger.warning(f"Could not cancel execution {execution_id}: {e}")
        return False


async def start_workflow(workflow_name: str, inputs: dict[str, Any]) -> str:
    """
    Start a workflow execution.

    Args:
        workflow_name: Name of the workflow to execute (matches .flow filename)
        inputs: Input parameters for the workflow

    Returns:
        Workflow execution ID

    Example:
        >>> workflow_id = await rhythm.start_workflow(
        ...     "processOrder",
        ...     inputs={"orderId": "order-123", "amount": 99.99}
        ... )
    """
    execution_id = RustBridge.start_workflow(workflow_name, inputs)
    logger.info(f"Started workflow {workflow_name} with ID {execution_id}")
    return execution_id


async def list_executions(
    queue: Optional[str] = None,
    status: Optional[str] = None,
    limit: int = 100,
    offset: int = 0,
) -> list[dict]:
    """
    List executions with optional filters.

    NOTE: This function is currently not implemented as it requires direct database access.
    Use the Rust bridge functions instead for execution management.

    Args:
        queue: Filter by queue name
        status: Filter by status
        limit: Maximum number of results
        offset: Offset for pagination

    Returns:
        List of execution dicts
    """
    raise NotImplementedError(
        "list_executions is not yet implemented. Use Rust bridge functions for execution management."
    )

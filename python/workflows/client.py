"""Client API for enqueuing work and sending signals"""

import json
import logging
from typing import Any, Optional
from datetime import datetime

from workflows.rust_bridge import RustBridge

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
    Enqueue an execution (job, activity, or workflow).

    Args:
        exec_type: Type of execution ('job', 'activity', 'workflow')
        function_name: Fully qualified function name
        args: Positional arguments
        kwargs: Keyword arguments
        queue: Queue name
        priority: Priority (0-10, higher = more urgent)
        max_retries: Maximum retry attempts
        timeout_seconds: Timeout in seconds
        parent_workflow_id: Parent workflow ID (for activities)

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


async def send_signal(workflow_id: str, signal_name: str, payload: dict[str, Any] = None) -> str:
    """
    Send a signal to a workflow.

    Args:
        workflow_id: The workflow execution ID
        signal_name: Name of the signal
        payload: Signal payload data

    Returns:
        Signal ID

    Example:
        await send_signal(workflow_id, "approved", {"approved": True, "approver": "user@example.com"})
    """
    payload = payload or {}
    signal_id = RustBridge.send_signal(workflow_id, signal_name, payload)
    logger.info(f"Signal {signal_name} sent to workflow {workflow_id}")
    return signal_id


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
            retry=False
        )
        logger.info(f"Execution {execution_id} cancelled")
        return True
    except Exception as e:
        logger.warning(f"Could not cancel execution {execution_id}: {e}")
        return False


async def list_executions(
    queue: Optional[str] = None,
    status: Optional[str] = None,
    limit: int = 100,
    offset: int = 0,
) -> list[dict]:
    """
    List executions with optional filters.

    Args:
        queue: Filter by queue name
        status: Filter by status
        limit: Maximum number of results
        offset: Offset for pagination

    Returns:
        List of execution dicts
    """
    query = """
        SELECT id, type, function_name, queue, status, priority,
               result, error, attempt, max_retries,
               created_at, claimed_at, completed_at
        FROM executions
        WHERE 1=1
    """
    params = []

    if queue:
        params.append(queue)
        query += f" AND queue = ${len(params)}"

    if status:
        params.append(status)
        query += f" AND status = ${len(params)}"

    query += f" ORDER BY created_at DESC LIMIT {limit} OFFSET {offset}"

    async with get_connection() as conn:
        rows = await conn.fetch(query, *params)

        results = []
        for row in rows:
            data = dict(row)

            # Parse JSON fields
            if data.get("result"):
                data["result"] = json.loads(data["result"]) if isinstance(data["result"], str) else data["result"]
            if data.get("error"):
                data["error"] = json.loads(data["error"]) if isinstance(data["error"], str) else data["error"]

            results.append(data)

        return results

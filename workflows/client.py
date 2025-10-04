"""Client API for enqueuing work and sending signals"""

import json
import logging
from typing import Any, Optional
from datetime import datetime

from workflows.db import get_connection
from workflows.utils import generate_id

logger = logging.getLogger(__name__)


async def enqueue_execution(
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
    execution_id = generate_id(exec_type[:3])  # job/act/wor prefix

    async with get_connection() as conn:
        async with conn.transaction():
            # Insert execution
            await conn.execute(
                """
                INSERT INTO executions (
                    id, type, function_name, queue, status, priority,
                    args, kwargs, max_retries, timeout_seconds, parent_workflow_id
                )
                VALUES ($1, $2, $3, $4, 'pending', $5, $6, $7, $8, $9, $10)
                """,
                execution_id,
                exec_type,
                function_name,
                queue,
                priority,
                json.dumps(list(args)),
                json.dumps(kwargs),
                max_retries,
                timeout_seconds,
                parent_workflow_id,
            )

            # Notify workers
            await conn.execute(f"NOTIFY queue_{queue}")

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
    signal_id = generate_id("sig")
    payload = payload or {}

    async with get_connection() as conn:
        async with conn.transaction():
            # Insert signal
            await conn.execute(
                """
                INSERT INTO workflow_signals (id, workflow_id, signal_name, payload, consumed)
                VALUES ($1, $2, $3, $4, FALSE)
                """,
                signal_id,
                workflow_id,
                signal_name,
                json.dumps(payload),
            )

            # Check if workflow is waiting for this signal
            workflow = await conn.fetchrow(
                """
                SELECT id, status, checkpoint, queue
                FROM executions
                WHERE id = $1 AND type = 'workflow'
                """,
                workflow_id,
            )

            if not workflow:
                logger.warning(f"Workflow {workflow_id} not found")
                return signal_id

            # If workflow is suspended and waiting for this signal, resume it
            checkpoint = json.loads(workflow["checkpoint"]) if workflow["checkpoint"] else {}
            pending_commands = checkpoint.get("pending_commands", [])

            # Check if any pending command is waiting for this signal
            signal_found = False
            for cmd in pending_commands:
                if cmd.get("type") == "wait_signal" and cmd.get("signal_name") == signal_name:
                    signal_found = True
                    break

            if signal_found and workflow["status"] == "suspended":
                # Add signal to history and resume workflow
                history = checkpoint.get("history", [])
                history.append(
                    {
                        "type": "signal",
                        "signal_name": signal_name,
                        "payload": payload,
                        "signal_id": signal_id,
                    }
                )

                checkpoint["history"] = history
                checkpoint["pending_commands"] = []  # Clear pending commands

                await conn.execute(
                    """
                    UPDATE executions
                    SET status = 'pending',
                        checkpoint = $2
                    WHERE id = $1
                    """,
                    workflow_id,
                    json.dumps(checkpoint),
                )

                # Mark signal as consumed
                await conn.execute(
                    "UPDATE workflow_signals SET consumed = TRUE WHERE id = $1", signal_id
                )

                # Notify queue
                await conn.execute(f"NOTIFY queue_{workflow['queue']}")

                logger.info(f"Signal {signal_name} sent to workflow {workflow_id}, workflow resumed")
            else:
                logger.info(
                    f"Signal {signal_name} sent to workflow {workflow_id}, but workflow not waiting"
                )

    return signal_id


async def get_execution_status(execution_id: str) -> Optional[dict]:
    """
    Get the status of an execution.

    Args:
        execution_id: The execution ID

    Returns:
        Execution status dict or None if not found
    """
    async with get_connection() as conn:
        row = await conn.fetchrow(
            """
            SELECT id, type, function_name, queue, status, priority,
                   result, error, attempt, max_retries,
                   created_at, claimed_at, completed_at
            FROM executions
            WHERE id = $1
            """,
            execution_id,
        )

        if not row:
            return None

        data = dict(row)

        # Parse JSON fields
        if data.get("result"):
            data["result"] = json.loads(data["result"]) if isinstance(data["result"], str) else data["result"]
        if data.get("error"):
            data["error"] = json.loads(data["error"]) if isinstance(data["error"], str) else data["error"]

        return data


async def cancel_execution(execution_id: str) -> bool:
    """
    Cancel a pending or suspended execution.

    Args:
        execution_id: The execution ID

    Returns:
        True if cancelled, False if not found or already completed/running
    """
    async with get_connection() as conn:
        result = await conn.execute(
            """
            UPDATE executions
            SET status = 'failed',
                error = $2,
                completed_at = NOW()
            WHERE id = $1
              AND status IN ('pending', 'suspended')
            """,
            execution_id,
            json.dumps({"message": "Execution cancelled", "type": "CancellationError"}),
        )

        cancelled = result and int(result.split()[-1]) > 0

        if cancelled:
            logger.info(f"Execution {execution_id} cancelled")
        else:
            logger.warning(f"Could not cancel execution {execution_id} (not found or not cancellable)")

        return cancelled


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

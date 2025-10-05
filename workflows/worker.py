"""Worker implementation for executing jobs, activities, and workflows"""

import asyncio
import logging
import signal
import sys
from datetime import datetime, timedelta
from typing import Optional
import traceback
import json

from workflows.config import settings
from workflows.db import get_connection, get_pool
from workflows.models import Execution, ExecutionStatus, ExecutionType, WorkerStatus
from workflows.registry import get_function
from workflows.context import (
    WorkflowExecutionContext,
    WorkflowSuspendException,
    set_current_workflow_context,
    clear_current_workflow_context,
)
from workflows.utils import generate_id, calculate_retry_delay

logger = logging.getLogger(__name__)


class Worker:
    """Worker that polls for and executes jobs, activities, and workflows"""

    def __init__(self, queues: list[str], worker_id: Optional[str] = None):
        self.worker_id = worker_id or generate_id("worker")
        self.queues = queues
        self.running = False
        self.current_executions = 0
        self.listener_task: Optional[asyncio.Task] = None
        self.heartbeat_task: Optional[asyncio.Task] = None
        self.poll_task: Optional[asyncio.Task] = None
        self.recovery_task: Optional[asyncio.Task] = None

        logger.info(f"Worker {self.worker_id} initialized for queues: {queues}")

    async def start(self):
        """Start the worker"""
        self.running = True
        logger.info(f"Worker {self.worker_id} starting...")

        # Register signal handlers
        self._setup_signal_handlers()

        # Start background tasks
        self.heartbeat_task = asyncio.create_task(self._heartbeat_loop())
        self.listener_task = asyncio.create_task(self._listener_loop())
        self.poll_task = asyncio.create_task(self._poll_loop())
        self.recovery_task = asyncio.create_task(self._recovery_loop())

        # Wait for shutdown
        try:
            await asyncio.gather(
                self.heartbeat_task,
                self.listener_task,
                self.poll_task,
                self.recovery_task,
            )
        except asyncio.CancelledError:
            logger.info("Worker tasks cancelled")

    async def stop(self):
        """Stop the worker gracefully"""
        logger.info(f"Worker {self.worker_id} stopping...")
        self.running = False

        # Cancel background tasks
        for task in [self.heartbeat_task, self.listener_task, self.poll_task, self.recovery_task]:
            if task and not task.done():
                task.cancel()

        # Update worker status
        await self._update_heartbeat(status=WorkerStatus.STOPPED)

        # Wait for current executions to complete (with timeout)
        timeout = 30
        start = asyncio.get_event_loop().time()
        while self.current_executions > 0:
            if asyncio.get_event_loop().time() - start > timeout:
                logger.warning(f"Timeout waiting for executions to complete")
                break
            await asyncio.sleep(0.5)

        logger.info(f"Worker {self.worker_id} stopped")

    def _setup_signal_handlers(self):
        """Setup signal handlers for graceful shutdown"""

        def handle_signal(signum, frame):
            logger.info(f"Received signal {signum}, initiating graceful shutdown...")
            asyncio.create_task(self.stop())

        signal.signal(signal.SIGINT, handle_signal)
        signal.signal(signal.SIGTERM, handle_signal)

    async def _heartbeat_loop(self):
        """Continuously update worker heartbeat"""
        while self.running:
            try:
                await self._update_heartbeat(status=WorkerStatus.RUNNING)
                await asyncio.sleep(settings.worker_heartbeat_interval)
            except asyncio.CancelledError:
                break
            except Exception as e:
                logger.error(f"Error in heartbeat loop: {e}")
                await asyncio.sleep(settings.worker_heartbeat_interval)

    async def _update_heartbeat(self, status: WorkerStatus = WorkerStatus.RUNNING):
        """Update worker heartbeat in database"""
        async with get_connection() as conn:
            await conn.execute(
                """
                INSERT INTO worker_heartbeats (worker_id, last_heartbeat, queues, status, metadata)
                VALUES ($1, $2, $3, $4, $5)
                ON CONFLICT (worker_id) DO UPDATE SET
                    last_heartbeat = $2,
                    queues = $3,
                    status = $4,
                    metadata = $5
                """,
                self.worker_id,
                datetime.utcnow(),
                self.queues,
                status.value,
                json.dumps({"current_executions": self.current_executions}),
            )

    async def _listener_loop(self):
        """Listen for NOTIFY events on queue channels"""
        pool = await get_pool()

        async with pool.acquire() as conn:
            # Listen on all queue channels
            for queue in self.queues:
                await conn.add_listener(f"queue_{queue}", lambda *args: None)

            logger.info(f"Worker {self.worker_id} listening on channels: {self.queues}")

            while self.running:
                try:
                    # Wait briefly and try to claim work
                    await asyncio.sleep(0.1)
                    await self._try_claim_and_execute()

                except asyncio.CancelledError:
                    break
                except Exception as e:
                    logger.error(f"Error in listener loop: {e}")
                    await asyncio.sleep(1)

    async def _poll_loop(self):
        """Fallback polling loop in case notifications are missed"""
        while self.running:
            try:
                await asyncio.sleep(settings.worker_poll_interval)
                await self._try_claim_and_execute()
            except asyncio.CancelledError:
                break
            except Exception as e:
                logger.error(f"Error in poll loop: {e}")
                await asyncio.sleep(settings.worker_poll_interval)

    async def _recovery_loop(self):
        """Periodically check for dead workers and recover their work"""
        while self.running:
            try:
                await asyncio.sleep(settings.worker_heartbeat_timeout)
                await self._recover_dead_worker_executions()
            except asyncio.CancelledError:
                break
            except Exception as e:
                logger.error(f"Error in recovery loop: {e}")
                await asyncio.sleep(settings.worker_heartbeat_timeout)

    async def _recover_dead_worker_executions(self):
        """Find and recover executions from dead workers"""
        timeout_threshold = datetime.utcnow() - timedelta(
            seconds=settings.worker_heartbeat_timeout
        )

        async with get_connection() as conn:
            # Find dead workers
            dead_workers = await conn.fetch(
                """
                SELECT worker_id FROM worker_heartbeats
                WHERE status = 'running'
                  AND last_heartbeat < $1
                """,
                timeout_threshold,
            )

            if not dead_workers:
                return

            dead_worker_ids = [w["worker_id"] for w in dead_workers]
            logger.warning(f"Found {len(dead_worker_ids)} dead workers: {dead_worker_ids}")

            # Mark workers as stopped
            await conn.execute(
                """
                UPDATE worker_heartbeats
                SET status = 'stopped'
                WHERE worker_id = ANY($1)
                """,
                dead_worker_ids,
            )

            # Reset their running executions to pending
            result = await conn.execute(
                """
                UPDATE executions
                SET status = 'pending', worker_id = NULL, claimed_at = NULL
                WHERE worker_id = ANY($1)
                  AND status IN ('running', 'suspended')
                """,
                dead_worker_ids,
            )

            recovered_count = int(result.split()[-1]) if result else 0
            if recovered_count > 0:
                logger.info(f"Recovered {recovered_count} executions from dead workers")

    async def _try_claim_and_execute(self):
        """Try to claim work and execute it"""
        if self.current_executions >= settings.worker_max_concurrent:
            return

        # Claim a pending execution
        execution = await self._claim_execution()
        if execution:
            # Execute in background task
            asyncio.create_task(self._execute_with_tracking(execution))

    async def _claim_execution(self) -> Optional[Execution]:
        """Claim a pending execution from the queue"""
        async with get_connection() as conn:
            row = await conn.fetchrow(
                """
                UPDATE executions
                SET status = 'running',
                    worker_id = $1,
                    claimed_at = NOW()
                WHERE id IN (
                    SELECT id FROM executions
                    WHERE queue = ANY($2)
                      AND status = 'pending'
                    ORDER BY priority DESC, created_at ASC
                    LIMIT 1
                    FOR UPDATE SKIP LOCKED
                )
                RETURNING *
                """,
                self.worker_id,
                self.queues,
            )

            if row:
                execution = Execution.from_record(row)
                logger.info(
                    f"Claimed {execution.type} execution {execution.id}: {execution.function_name}"
                )
                return execution

        return None

    async def _execute_with_tracking(self, execution: Execution):
        """Execute with tracking of concurrent executions"""
        self.current_executions += 1
        try:
            await self._execute(execution)
        finally:
            self.current_executions -= 1

    async def _execute(self, execution: Execution):
        """Execute a job, activity, or workflow"""
        try:
            logger.info(f"Executing {execution.type} {execution.id}: {execution.function_name}")

            # Get the function
            fn = get_function(execution.function_name)

            # Execute based on type
            if execution.type == ExecutionType.WORKFLOW:
                await self._execute_workflow(execution, fn)
            else:
                await self._execute_function(execution, fn)

        except Exception as e:
            logger.error(f"Error executing {execution.id}: {e}\n{traceback.format_exc()}")
            await self._handle_execution_failure(execution, e)

    async def _execute_function(self, execution: Execution, fn):
        """Execute a simple function (job or activity)"""
        try:
            # Set timeout
            timeout = execution.timeout_seconds or settings.default_timeout

            # Execute with timeout
            result = await asyncio.wait_for(
                fn(*execution.args, **execution.kwargs), timeout=timeout
            )

            # Mark as completed
            await self._complete_execution(execution, result)

        except asyncio.TimeoutError:
            raise TimeoutError(f"Execution timed out after {timeout} seconds")

    async def _execute_workflow(self, execution: Execution, fn):
        """Execute a workflow with replay support"""
        # Create workflow context
        ctx = WorkflowExecutionContext(execution.id, execution.checkpoint)
        set_current_workflow_context(ctx)

        try:
            # Set timeout
            timeout = execution.timeout_seconds or settings.default_workflow_timeout

            # Execute workflow function with timeout
            result = await asyncio.wait_for(
                fn(*execution.args, **execution.kwargs), timeout=timeout
            )

            # If we got here, workflow completed
            await self._complete_execution(execution, result)

        except WorkflowSuspendException as e:
            # Workflow suspended - handle commands
            await self._handle_workflow_suspend(execution, ctx, e.commands)

        except asyncio.TimeoutError:
            raise TimeoutError(f"Workflow timed out after {timeout} seconds")

        finally:
            clear_current_workflow_context()

    async def _handle_workflow_suspend(
        self, execution: Execution, ctx: WorkflowExecutionContext, commands: list[dict]
    ):
        """Handle workflow suspension and create activity executions"""
        logger.info(f"Workflow {execution.id} suspended with {len(commands)} commands")

        async with get_connection() as conn:
            async with conn.transaction():
                # Create activity executions for each command
                for cmd in commands:
                    if cmd["type"] == "activity":
                        await conn.execute(
                            """
                            INSERT INTO executions (
                                id, type, function_name, queue, status, priority,
                                args, kwargs, options, max_retries, timeout_seconds,
                                parent_workflow_id
                            )
                            VALUES ($1, 'activity', $2, $3, 'pending', $4, $5, $6, $7, $8, $9, $10)
                            """,
                            cmd["activity_execution_id"],
                            cmd["name"],
                            execution.queue,  # Inherit workflow's queue
                            cmd["config"].get("priority", 5),
                            json.dumps(cmd["args"]),
                            json.dumps(cmd["kwargs"]),
                            json.dumps(cmd["config"]),
                            cmd["config"].get("retries", settings.default_retries),
                            cmd["config"].get("timeout"),
                            execution.id,
                        )

                    elif cmd["type"] == "wait_signal":
                        # Just update checkpoint - signal waiting is passive
                        pass

                # Update workflow checkpoint
                new_checkpoint = {
                    "history": ctx.history,
                    "pending_commands": commands,
                }

                await conn.execute(
                    """
                    UPDATE executions
                    SET status = 'suspended',
                        checkpoint = $2,
                        worker_id = NULL
                    WHERE id = $1
                    """,
                    execution.id,
                    json.dumps(new_checkpoint),
                )

                # Notify queue for new activities
                await conn.execute(f"NOTIFY queue_{execution.queue}")

        logger.info(f"Workflow {execution.id} suspended and activities created")

    async def _complete_execution(self, execution: Execution, result: any):
        """Mark execution as completed"""
        logger.info(f"Execution {execution.id} completed successfully")

        async with get_connection() as conn:
            async with conn.transaction():
                # Update execution
                await conn.execute(
                    """
                    UPDATE executions
                    SET status = 'completed',
                        result = $2,
                        completed_at = NOW(),
                        worker_id = NULL
                    WHERE id = $1
                    """,
                    execution.id,
                    json.dumps(result),
                )

                # If this is an activity, resume parent workflow
                if execution.parent_workflow_id:
                    await self._resume_parent_workflow(conn, execution, result)

    async def _resume_parent_workflow(self, conn, activity_execution: Execution, result: any):
        """Resume parent workflow after activity completion"""
        workflow_id = activity_execution.parent_workflow_id

        # Append result to workflow history
        workflow = await conn.fetchrow(
            "SELECT checkpoint FROM executions WHERE id = $1", workflow_id
        )

        if not workflow:
            logger.error(f"Parent workflow {workflow_id} not found")
            return

        checkpoint = json.loads(workflow["checkpoint"]) if workflow["checkpoint"] else {}
        history = checkpoint.get("history", [])

        history.append(
            {
                "type": "activity",
                "name": activity_execution.function_name,
                "result": result,
                "activity_execution_id": activity_execution.id,
            }
        )

        checkpoint["history"] = history

        # Re-queue workflow
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

        # Notify queue
        queue = await conn.fetchval("SELECT queue FROM executions WHERE id = $1", workflow_id)
        if queue:
            await conn.execute(f"NOTIFY queue_{queue}")

        logger.info(f"Parent workflow {workflow_id} resumed")

    async def _handle_execution_failure(self, execution: Execution, error: Exception):
        """Handle execution failure with retry logic"""
        execution.attempt += 1

        error_data = {
            "message": str(error),
            "type": type(error).__name__,
            "traceback": traceback.format_exc(),
        }

        logger.error(
            f"Execution {execution.id} failed (attempt {execution.attempt}/{execution.max_retries}): {error}"
        )

        async with get_connection() as conn:
            if execution.attempt < execution.max_retries:
                # Retry with backoff
                delay = calculate_retry_delay(execution.attempt)
                retry_at = datetime.utcnow() + timedelta(seconds=delay)

                await conn.execute(
                    """
                    UPDATE executions
                    SET status = 'pending',
                        attempt = $2,
                        error = $3,
                        worker_id = NULL,
                        claimed_at = NULL
                    WHERE id = $1
                    """,
                    execution.id,
                    execution.attempt,
                    json.dumps(error_data),
                )

                logger.info(f"Execution {execution.id} will retry in {delay}s")

            else:
                # Max retries exhausted
                await conn.execute(
                    """
                    UPDATE executions
                    SET status = 'failed',
                        error = $2,
                        completed_at = NOW(),
                        worker_id = NULL
                    WHERE id = $1
                    """,
                    execution.id,
                    json.dumps(error_data),
                )

                logger.error(f"Execution {execution.id} failed permanently after {execution.attempt} attempts")


async def run_worker(queues: list[str], worker_id: Optional[str] = None):
    """Run a worker (main entry point)"""
    worker = Worker(queues, worker_id)

    try:
        await worker.start()
    except KeyboardInterrupt:
        logger.info("Received interrupt signal")
    finally:
        await worker.stop()

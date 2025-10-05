"""Worker implementation for executing jobs, activities, and workflows"""

import asyncio
import logging
import signal
import sys
from datetime import datetime, timedelta
from typing import Optional
import traceback
import json

from currant.config import settings
from currant.rust_bridge import RustBridge
from currant.models import Execution, ExecutionStatus, ExecutionType, WorkerStatus
from currant.registry import get_function
from currant.context import (
    WorkflowExecutionContext,
    WorkflowSuspendException,
    set_current_workflow_context,
    clear_current_workflow_context,
)
from currant.utils import generate_id, calculate_retry_delay

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

        # Update worker status via Rust
        RustBridge.stop_worker(self.worker_id)

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
        """Update worker heartbeat in database via Rust"""
        RustBridge.update_heartbeat(self.worker_id, self.queues)

    async def _listener_loop(self):
        """Continuously try to claim and execute work"""
        logger.info(f"Worker {self.worker_id} listening on queues: {self.queues}")

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
        """Find and recover executions from dead workers via Rust"""
        recovered_count = RustBridge.recover_dead_workers(settings.worker_heartbeat_timeout)
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
        """Claim a pending execution from the queue via Rust"""
        exec_dict = RustBridge.claim_execution(self.worker_id, self.queues)
        if exec_dict:
            execution = Execution.from_dict(exec_dict)
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
        """Handle workflow suspension and create activity executions via Rust"""
        logger.info(f"Workflow {execution.id} suspended with {len(commands)} commands")

        # Create activity executions for each command
        for cmd in commands:
            if cmd["type"] == "activity":
                RustBridge.create_execution(
                    exec_type="activity",
                    function_name=cmd["name"],
                    queue=execution.queue,  # Inherit workflow's queue
                    priority=cmd["config"].get("priority", 5),
                    args=cmd["args"],
                    kwargs=cmd["kwargs"],
                    max_retries=cmd["config"].get("retries", settings.default_retries),
                    timeout_seconds=cmd["config"].get("timeout"),
                    parent_workflow_id=execution.id,
                )

            elif cmd["type"] == "wait_signal":
                # Just update checkpoint - signal waiting is passive
                pass

        # Update workflow checkpoint and suspend
        new_checkpoint = {
            "history": ctx.history,
            "pending_commands": commands,
        }

        RustBridge.suspend_workflow(execution.id, new_checkpoint)

        logger.info(f"Workflow {execution.id} suspended and activities created")

    async def _complete_execution(self, execution: Execution, result: any):
        """Mark execution as completed via Rust"""
        logger.info(f"Execution {execution.id} completed successfully")

        RustBridge.complete_execution(execution.id, result)

        # If this is an activity, resume parent workflow
        if execution.parent_workflow_id:
            await self._resume_parent_workflow(execution, result)

    async def _resume_parent_workflow(self, activity_execution: Execution, result: any):
        """Resume parent workflow after activity completion"""
        workflow_id = activity_execution.parent_workflow_id

        # Get workflow checkpoint
        workflow_dict = RustBridge.get_execution(workflow_id)
        if not workflow_dict:
            logger.error(f"Parent workflow {workflow_id} not found")
            return

        checkpoint = workflow_dict.get("checkpoint") or {}
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

        # Suspend workflow with updated checkpoint (this will set status to suspended)
        RustBridge.suspend_workflow(workflow_id, checkpoint)

        # Immediately resume it (set status back to pending)
        RustBridge.resume_workflow(workflow_id)

        logger.info(f"Parent workflow {workflow_id} resumed")

    async def _handle_execution_failure(self, execution: Execution, error: Exception):
        """Handle execution failure with retry logic via Rust"""
        execution.attempt += 1

        error_data = {
            "message": str(error),
            "type": type(error).__name__,
            "traceback": traceback.format_exc(),
        }

        logger.error(
            f"Execution {execution.id} failed (attempt {execution.attempt}/{execution.max_retries}): {error}"
        )

        if execution.attempt < execution.max_retries:
            # Retry
            delay = calculate_retry_delay(execution.attempt)
            RustBridge.fail_execution(execution.id, error_data, retry=True)
            logger.info(f"Execution {execution.id} will retry in {delay}s")
        else:
            # Max retries exhausted
            RustBridge.fail_execution(execution.id, error_data, retry=False)
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

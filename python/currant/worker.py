"""Worker implementation for executing tasks and workflows"""

import asyncio
import asyncpg
import logging
import signal
from typing import Optional, List
import traceback
import os

from currant.config import settings
from currant.rust_bridge import RustBridge
from currant.models import Execution, ExecutionType, WorkerStatus
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
    """Worker that polls for and executes tasks and workflows"""

    def __init__(self, queues: list[str], worker_id: Optional[str] = None):
        self.worker_id = worker_id or generate_id("worker")
        self.queues = queues
        self.running = False
        self.current_executions = 0
        self.listener_task: Optional[asyncio.Task] = None
        self.heartbeat_task: Optional[asyncio.Task] = None
        self.poll_task: Optional[asyncio.Task] = None
        self.recovery_task: Optional[asyncio.Task] = None
        self.claimer_task: Optional[asyncio.Task] = None
        self.notify_conn: Optional[asyncpg.Connection] = None
        self.notify_event: asyncio.Event = asyncio.Event()

        # Local task queue for prefetching
        self.local_queue: asyncio.Queue = asyncio.Queue(maxsize=settings.worker_max_concurrent * 2)
        self.semaphore: asyncio.Semaphore = asyncio.Semaphore(settings.worker_max_concurrent)

        # Completion batching
        self.completion_queue: List[tuple[str, any]] = []
        self.completion_lock: asyncio.Lock = asyncio.Lock()
        self.completer_task: Optional[asyncio.Task] = None

        logger.info(f"Worker {self.worker_id} initialized for queues: {queues}")

    async def start(self):
        """Start the worker"""
        self.running = True
        logger.info(f"Worker {self.worker_id} starting...")

        # Register signal handlers
        self._setup_signal_handlers()

        # Setup LISTEN connection
        await self._setup_listen_connection()

        # Start background tasks
        self.heartbeat_task = asyncio.create_task(self._heartbeat_loop())
        self.listener_task = asyncio.create_task(self._listener_loop())
        self.poll_task = asyncio.create_task(self._poll_loop())
        self.recovery_task = asyncio.create_task(self._recovery_loop())
        self.claimer_task = asyncio.create_task(self._claimer_loop())
        self.completer_task = asyncio.create_task(self._completer_loop())

        # Wait for shutdown
        try:
            await asyncio.gather(
                self.heartbeat_task,
                self.listener_task,
                self.poll_task,
                self.recovery_task,
                self.claimer_task,
                self.completer_task,
            )
        except asyncio.CancelledError:
            logger.info("Worker tasks cancelled")

    async def stop(self):
        """Stop the worker gracefully"""
        logger.info(f"Worker {self.worker_id} stopping...")
        self.running = False

        # Cancel background tasks
        for task in [self.heartbeat_task, self.listener_task, self.poll_task, self.recovery_task, self.claimer_task, self.completer_task]:
            if task and not task.done():
                task.cancel()

        # Flush any pending completions
        await self._flush_completions()

        # Close LISTEN connection
        if self.notify_conn:
            await self.notify_conn.close()

        # Update worker status via Rust
        RustBridge.stop_worker(self.worker_id)

        # Wait for current executions to complete (with timeout)
        timeout = 30
        start = asyncio.get_event_loop().time()
        while self.current_executions > 0:
            if asyncio.get_event_loop().time() - start > timeout:
                logger.warning("Timeout waiting for executions to complete")
                break
            await asyncio.sleep(0.5)

        logger.info(f"Worker {self.worker_id} stopped")

    async def _setup_listen_connection(self):
        """Setup dedicated connection for LISTEN/NOTIFY"""
        try:
            db_url = os.getenv("CURRANT_DATABASE_URL")
            if not db_url:
                logger.warning("CURRANT_DATABASE_URL not set, LISTEN/NOTIFY disabled")
                return

            self.notify_conn = await asyncpg.connect(db_url)

            # Setup notification callback
            def on_notification(conn, pid, channel, payload):
                self.notify_event.set()

            # Listen on all queues
            for queue in self.queues:
                await self.notify_conn.add_listener(queue, on_notification)
                logger.info(f"Listening on queue: {queue}")

        except Exception as e:
            logger.warning(f"Failed to setup LISTEN connection: {e}, falling back to polling")
            self.notify_conn = None

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
        """Pull tasks from local queue and execute them"""
        logger.info(f"Worker {self.worker_id} executing from local queue")

        while self.running:
            try:
                # Wait for a task from the local queue (with timeout)
                execution = await asyncio.wait_for(self.local_queue.get(), timeout=1.0)

                # Execute with semaphore for concurrency control
                asyncio.create_task(self._execute_with_semaphore(execution))

            except asyncio.TimeoutError:
                # No tasks in queue, just loop again
                continue
            except asyncio.CancelledError:
                break
            except Exception as e:
                logger.error(f"Error in executor loop: {e}")
                await asyncio.sleep(1)

    async def _claimer_loop(self):
        """Continuously claim tasks and fill the local queue"""
        logger.info(f"Worker {self.worker_id} starting claimer loop")

        while self.running:
            try:
                # Check queue space first
                queue_space = self.local_queue.maxsize - self.local_queue.qsize()

                if queue_space > 0:
                    # Claim up to the available space
                    claimed = await self._claim_executions_batch(queue_space)
                    for execution in claimed:
                        await self.local_queue.put(execution)

                    # If we claimed fewer than requested, no more tasks available
                    # Wait for notification before trying again
                    if len(claimed) < queue_space:
                        if self.notify_conn:
                            try:
                                await asyncio.wait_for(self.notify_event.wait(), timeout=5.0)
                                self.notify_event.clear()
                            except asyncio.TimeoutError:
                                pass
                        else:
                            await asyncio.sleep(1.0)
                else:
                    # Queue full, wait for space
                    await asyncio.sleep(0.1)

            except asyncio.CancelledError:
                break
            except Exception as e:
                logger.error(f"Error in claimer loop: {e}")
                await asyncio.sleep(1)

    async def _poll_loop(self):
        """Fallback polling loop in case notifications are missed"""
        while self.running:
            try:
                # Poll less aggressively when LISTEN/NOTIFY is working
                poll_interval = 5.0 if self.notify_conn else settings.worker_poll_interval
                await asyncio.sleep(poll_interval)
                # Trigger the claimer to check for work
                self.notify_event.set()
            except asyncio.CancelledError:
                break
            except Exception as e:
                logger.error(f"Error in poll loop: {e}")
                await asyncio.sleep(5.0)

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

    async def _completer_loop(self):
        """Periodically batch and flush completion updates"""
        while self.running:
            try:
                # Flush every 1ms for low latency
                await asyncio.sleep(0.001)
                await self._flush_completions()
            except asyncio.CancelledError:
                break
            except Exception as e:
                logger.error(f"Error in completer loop: {e}")
                await asyncio.sleep(0.1)

    async def _recover_dead_worker_executions(self):
        """Find and recover executions from dead workers via Rust"""
        recovered_count = RustBridge.recover_dead_workers(settings.worker_heartbeat_timeout)
        if recovered_count > 0:
            logger.info(f"Recovered {recovered_count} executions from dead workers")

    async def _flush_completions(self):
        """Flush pending completions to database in batch"""
        async with self.completion_lock:
            if not self.completion_queue:
                return

            batch = self.completion_queue[:]
            self.completion_queue.clear()

        try:
            RustBridge.complete_executions_batch(batch)
            logger.debug(f"Flushed {len(batch)} completions")
        except Exception as e:
            logger.error(f"Error flushing completions: {e}")
            # Re-add to queue on failure
            async with self.completion_lock:
                self.completion_queue.extend(batch)

    async def _claim_executions_batch(self, limit: int) -> List[Execution]:
        """Claim multiple pending executions from the queue via Rust (batch claiming)"""
        exec_dicts = RustBridge.claim_executions_batch(self.worker_id, self.queues, limit)
        executions = []
        for exec_dict in exec_dicts:
            execution = Execution.from_dict(exec_dict)
            logger.info(
                f"Claimed {execution.type} execution {execution.id}: {execution.function_name}"
            )
            executions.append(execution)
        return executions

    async def _execute_with_semaphore(self, execution: Execution):
        """Execute with semaphore-based concurrency control"""
        async with self.semaphore:
            self.current_executions += 1
            try:
                await self._execute(execution)
            finally:
                self.current_executions -= 1

    async def _execute(self, execution: Execution):
        """Execute a task or workflow"""
        try:
            logger.info(f"Executing {execution.type} {execution.id}: {execution.function_name}")

            # Execute based on type
            if execution.type == ExecutionType.WORKFLOW:
                # Try to get Python function, but if not found, use DSL executor
                fn = get_function(execution.function_name, required=False)
                if fn:
                    # Python-based workflow (old style)
                    await self._execute_workflow(execution, fn)
                else:
                    # DSL-based workflow
                    await self._execute_dsl_workflow(execution)
            else:
                # Regular task - must have Python function
                fn = get_function(execution.function_name)
                await self._execute_function(execution, fn)

        except Exception as e:
            logger.error(f"Error executing {execution.id}: {e}\n{traceback.format_exc()}")
            await self._handle_execution_failure(execution, e)

    async def _execute_function(self, execution: Execution, fn):
        """Execute a simple task function"""
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

    async def _execute_dsl_workflow(self, execution: Execution):
        """Execute a DSL-based workflow by calling Rust executor"""
        logger.info(f"Executing DSL workflow {execution.id}")

        # Call Rust execute_workflow_step
        result_str = await asyncio.get_event_loop().run_in_executor(
            None,
            lambda: __import__('currant').currant_core.execute_workflow_step_sync(execution.id)
        )

        logger.info(f"DSL workflow step result: {result_str}")

        # Result will be "Suspended", "Completed", or "Continue"
        # The Rust code handles all state updates, we just need to log
        if "Completed" in result_str:
            logger.info(f"DSL workflow {execution.id} completed")
        elif "Suspended" in result_str:
            logger.info(f"DSL workflow {execution.id} suspended")
        elif "Continue" in result_str:
            # Workflow wants to continue immediately to next step
            # Schedule it to execute again
            logger.info(f"DSL workflow {execution.id} continuing to next step")

    async def _handle_workflow_suspend(
        self, execution: Execution, ctx: WorkflowExecutionContext, commands: list[dict]
    ):
        """Handle workflow suspension and create task executions via Rust"""
        logger.info(f"Workflow {execution.id} suspended with {len(commands)} commands")

        # Create task executions for each command
        for cmd in commands:
            if cmd["type"] == "task":
                RustBridge.create_execution(
                    exec_type="task",
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

        logger.info(f"Workflow {execution.id} suspended and child tasks created")

    async def _complete_execution(self, execution: Execution, result: any):
        """Mark execution as completed via Rust"""
        logger.info(f"Execution {execution.id} completed successfully")

        # Add to completion queue for batch processing
        async with self.completion_lock:
            self.completion_queue.append((execution.id, result))

        # If this is a workflow task, resume parent workflow
        if execution.parent_workflow_id:
            await self._resume_parent_workflow(execution, result)

    async def _resume_parent_workflow(self, task_execution: Execution, result: any):
        """Resume parent workflow after task completion"""
        workflow_id = task_execution.parent_workflow_id

        # Get workflow checkpoint
        workflow_dict = RustBridge.get_execution(workflow_id)
        if not workflow_dict:
            logger.error(f"Parent workflow {workflow_id} not found")
            return

        checkpoint = workflow_dict.get("checkpoint") or {}
        history = checkpoint.get("history", [])

        history.append(
            {
                "type": "task",
                "name": task_execution.function_name,
                "result": result,
                "task_execution_id": task_execution.id,
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
            logger.error(
                f"Execution {execution.id} failed permanently after {execution.attempt} attempts"
            )


async def run_worker(queues: list[str], worker_id: Optional[str] = None):
    """Run a worker (main entry point)"""
    worker = Worker(queues, worker_id)

    try:
        await worker.start()
    except KeyboardInterrupt:
        logger.info("Received interrupt signal")
    finally:
        await worker.stop()

"""Worker implementation for executing tasks and workflows"""

import asyncio
import logging
import signal
from typing import Optional, List
import traceback

from rhythm.config import settings
from rhythm.rust_bridge import RustBridge
from rhythm.models import Execution, ExecutionType, WorkerStatus
from rhythm.registry import get_function
from rhythm.utils import generate_id, calculate_retry_delay
from rhythm import rhythm_core

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
        self.claimer_task: Optional[asyncio.Task] = None
        self.recovery_task: Optional[asyncio.Task] = None

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

        # Start background tasks
        self.heartbeat_task = asyncio.create_task(self._heartbeat_loop())
        self.listener_task = asyncio.create_task(self._listener_loop())
        self.claimer_task = asyncio.create_task(self._claimer_loop())
        self.recovery_task = asyncio.create_task(self._recovery_loop())
        self.completer_task = asyncio.create_task(self._completer_loop())

        # Wait for shutdown
        try:
            await asyncio.gather(
                self.heartbeat_task,
                self.listener_task,
                self.claimer_task,
                self.recovery_task,
                self.completer_task,
            )
        except asyncio.CancelledError:
            logger.info("Worker tasks cancelled")

    async def stop(self):
        """Stop the worker gracefully"""
        logger.info(f"Worker {self.worker_id} stopping...")
        self.running = False

        # Cancel background tasks
        for task in [self.heartbeat_task, self.listener_task, self.claimer_task, self.recovery_task, self.completer_task]:
            if task and not task.done():
                task.cancel()

        # Flush any pending completions
        await self._flush_completions()

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
                    # Wait before trying again (configured poll interval)
                    if len(claimed) < queue_space:
                        await asyncio.sleep(settings.worker_poll_interval)
                else:
                    # Queue full, wait for space
                    await asyncio.sleep(0.1)

            except asyncio.CancelledError:
                break
            except Exception as e:
                logger.error(f"Error in claimer loop: {e}")
                await asyncio.sleep(1)

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

            # Check for builtin task types first
            if execution.function_name == "builtin.resume_workflow":
                await self._execute_builtin_resume_workflow(execution)
            elif execution.type == ExecutionType.WORKFLOW:
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

    async def _execute_builtin_resume_workflow(self, execution: Execution):
        """Execute builtin.resume_workflow - resume a suspended workflow"""
        logger.info(f"Executing builtin.resume_workflow for execution {execution.id}")

        # Extract workflow_id from args (first element)
        if not execution.args or len(execution.args) == 0:
            raise ValueError("builtin.resume_workflow requires workflow_id in args[0]")

        workflow_id = execution.args[0]
        logger.info(f"Resuming workflow {workflow_id}")

        # Call Rust execute_workflow_step to resume the workflow
        result_str = await asyncio.get_event_loop().run_in_executor(
            None,
            lambda: rhythm_core.execute_workflow_step_sync(workflow_id)
        )

        logger.info(f"Resume workflow result: {result_str}")

        # Mark the resume task itself as completed
        # The workflow's state is managed by the Rust executor
        await self._complete_execution(execution, {"status": "resumed", "result": result_str})

    async def _execute_dsl_workflow(self, execution: Execution):
        """Execute a DSL-based workflow by calling Rust executor"""
        logger.info(f"Executing DSL workflow {execution.id}")

        # Keep executing steps while workflow wants to continue
        while True:
            # Call Rust execute_workflow_step
            result_str = await asyncio.get_event_loop().run_in_executor(
                None,
                lambda: rhythm_core.execute_workflow_step_sync(execution.id)
            )

            logger.info(f"DSL workflow step result: {result_str}")

            # Result will be "Suspended", "Completed", or "Continue"
            if "Completed" in result_str:
                logger.info(f"DSL workflow {execution.id} completed")
                break
            elif "Suspended" in result_str:
                logger.info(f"DSL workflow {execution.id} suspended")
                break
            elif "Continue" in result_str:
                # Workflow wants to continue immediately to next step
                # Loop and execute again
                logger.info(f"DSL workflow {execution.id} continuing to next step")
                continue
            else:
                # Unknown result, break to be safe
                logger.warning(f"Unknown workflow step result: {result_str}")
                break

    async def _complete_execution(self, execution: Execution, result: any):
        """Mark execution as completed via Rust"""
        logger.info(f"Execution {execution.id} completed successfully")

        # Add to completion queue for batch processing
        async with self.completion_lock:
            self.completion_queue.append((execution.id, result))

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
    # Ensure Rust bridge is initialized before starting worker
    # This initializes the database pool without running migrations
    try:
        RustBridge.initialize(auto_migrate=False, require_initialized=False)
    except Exception as e:
        logger.warning(f"Failed to initialize Rust bridge: {e}")

    worker = Worker(queues, worker_id)

    try:
        await worker.start()
    except KeyboardInterrupt:
        logger.info("Received interrupt signal")
    finally:
        await worker.stop()

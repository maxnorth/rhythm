"""Worker implementation for executing tasks and workflows"""

import asyncio
import logging
import signal
from typing import Optional, List
import traceback

from rhythm.config import settings
from rhythm.rust_bridge import RustBridge
from rhythm.models import Execution, ExecutionType
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
        self.executor_task: Optional[asyncio.Task] = None

        # Concurrency control
        self.semaphore: asyncio.Semaphore = asyncio.Semaphore(settings.worker_max_concurrent)

        logger.info(f"Worker {self.worker_id} initialized for queues: {queues}")

    async def start(self):
        """Start the worker"""
        self.running = True
        logger.info(f"Worker {self.worker_id} starting...")

        # Register signal handlers
        self._setup_signal_handlers()

        # Start background tasks
        self.executor_task = asyncio.create_task(self._executor_loop())

        # Wait for shutdown
        try:
            await self.executor_task
        except asyncio.CancelledError:
            logger.info("Worker tasks cancelled")

    async def stop(self):
        """Stop the worker gracefully"""
        logger.info(f"Worker {self.worker_id} stopping...")
        self.running = False

        # Cancel background tasks
        if self.executor_task and not self.executor_task.done():
            self.executor_task.cancel()

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

    async def _executor_loop(self):
        """Continuously claim and execute tasks"""
        logger.info(f"Worker {self.worker_id} starting executor loop")

        while self.running:
            try:
                # Claim a single execution
                exec_dict = RustBridge.claim_execution(self.worker_id, self.queues)

                if exec_dict:
                    execution = Execution.from_dict(exec_dict)
                    logger.info(
                        f"Claimed {execution.type} execution {execution.id}: {execution.function_name}"
                    )
                    # Execute with semaphore for concurrency control
                    asyncio.create_task(self._execute_with_semaphore(execution))
                else:
                    # No work available, wait before trying again
                    await asyncio.sleep(settings.worker_poll_interval)

            except asyncio.CancelledError:
                break
            except Exception as e:
                logger.error(f"Error in executor loop: {e}")
                await asyncio.sleep(1)

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

            # Execute with timeout - unpack inputs as keyword arguments
            result = await asyncio.wait_for(
                fn(**execution.inputs), timeout=timeout
            )

            # Mark as completed
            await self._complete_execution(execution, result)

        except asyncio.TimeoutError:
            raise TimeoutError(f"Execution timed out after {timeout} seconds")

    async def _execute_builtin_resume_workflow(self, execution: Execution):
        """Execute builtin.resume_workflow - resume a suspended workflow"""
        logger.info(f"Executing builtin.resume_workflow for execution {execution.id}")

        # Extract workflow_id from inputs
        workflow_id = execution.inputs.get("workflow_id")
        if not workflow_id:
            raise ValueError("builtin.resume_workflow requires workflow_id in inputs")

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
        RustBridge.complete_execution(execution.id, result)

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

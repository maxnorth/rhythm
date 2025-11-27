"""Worker implementation for executing tasks and workflows"""

import asyncio
import logging
import signal
import uuid
from typing import Optional
import traceback

from rhythm.config import settings
from rhythm.core_bridge import CoreBridge
from rhythm.models import Execution
from rhythm.registry import get_function

logger = logging.getLogger(__name__)


class Worker:
    """Worker that polls for and executes tasks and workflows"""

    def __init__(self, queues: list[str], worker_id: Optional[str] = None):
        self.worker_id = worker_id or f"worker_{uuid.uuid4()}"
        self.queues = queues
        self.running = False
        self.current_executions = 0
        self.executor_tasks: list[asyncio.Task] = []

        logger.info(f"Worker {self.worker_id} initialized for queues: {queues}")

    async def start(self):
        """Start the worker"""
        self.running = True
        logger.info(f"Worker {self.worker_id} starting...")

        # Register signal handlers
        self._setup_signal_handlers()

        # Start N parallel claim+execute loops (natural concurrency control)
        concurrency = settings.worker_max_concurrent
        logger.info(f"Starting {concurrency} parallel claim+execute loops")

        self.executor_tasks = [
            asyncio.create_task(self._claim_execute_loop(i))
            for i in range(concurrency)
        ]

        # Wait for shutdown
        try:
            await asyncio.gather(*self.executor_tasks)
        except asyncio.CancelledError:
            logger.info("Worker tasks cancelled")

    async def stop(self):
        """Stop the worker gracefully"""
        logger.info(f"Worker {self.worker_id} stopping...")
        self.running = False

        # Cancel background tasks
        for task in self.executor_tasks:
            if not task.done():
                task.cancel()

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

        def handle_signal(signum, _frame):
            logger.info(f"Received signal {signum}, initiating graceful shutdown...")
            asyncio.create_task(self.stop())

        signal.signal(signal.SIGINT, handle_signal)
        signal.signal(signal.SIGTERM, handle_signal)

    async def _claim_execute_loop(self, loop_id: int):
        """Claim and execute tasks in a loop (natural concurrency control)"""
        logger.info(f"Worker {self.worker_id} loop {loop_id} started")

        while self.running:
            try:
                # Claim a single execution
                execution = CoreBridge.claim_execution(self.worker_id, self.queues)

                if execution:
                    logger.info(
                        f"Loop {loop_id} claimed {execution.type} execution {execution.id}: {execution.function_name}"
                    )

                    # Execute immediately (blocks this loop until complete)
                    self.current_executions += 1
                    try:
                        await self._execute(execution)
                    finally:
                        self.current_executions -= 1
                else:
                    # No work available, wait before trying again
                    await asyncio.sleep(settings.worker_poll_interval)

            except asyncio.CancelledError:
                break
            except Exception as e:
                logger.error(f"Error in loop {loop_id}: {e}")
                await asyncio.sleep(1)

        logger.info(f"Worker {self.worker_id} loop {loop_id} stopped")

    async def _execute(self, execution: Execution):
        """Execute a task"""
        try:
            logger.info(f"Executing task {execution.id}: {execution.function_name}")

            # V2's claim_work handles workflows internally
            # We only receive tasks here
            fn = get_function(execution.function_name)
            await self._execute_function(execution, fn)

        except Exception as e:
            logger.error(f"Error executing {execution.id}: {e}\n{traceback.format_exc()}")
            await self._handle_execution_failure(execution, e)

    async def _execute_function(self, execution: Execution, fn):
        """Execute a task function (supports both sync and async)"""
        try:
            # Set timeout
            timeout = execution.timeout_seconds or settings.default_timeout

            # Check if function is async or sync
            if asyncio.iscoroutinefunction(fn):
                # Async function - await directly
                logger.debug(f"Executing async function {execution.function_name}")
                result = await asyncio.wait_for(
                    fn(**execution.inputs), timeout=timeout
                )
            else:
                # Sync function - run in thread pool to avoid blocking event loop
                logger.debug(f"Executing sync function {execution.function_name} in thread pool")
                result = await asyncio.wait_for(
                    asyncio.to_thread(fn, **execution.inputs), timeout=timeout
                )

            # Mark as completed
            await self._complete_execution(execution, result)

        except asyncio.TimeoutError:
            raise TimeoutError(f"Execution timed out after {timeout} seconds")

    async def _complete_execution(self, execution: Execution, result: any):
        """Mark execution as completed via core"""
        logger.info(f"Execution {execution.id} completed successfully")
        CoreBridge.complete_execution(execution.id, result)

    async def _handle_execution_failure(self, execution: Execution, error: Exception):
        """Handle execution failure - v2 manages retries automatically"""
        error_data = {
            "message": str(error),
            "type": type(error).__name__,
            "traceback": traceback.format_exc(),
        }

        logger.error(f"Execution {execution.id} failed: {error}")

        # Report the failure - v2 will handle retry logic automatically
        CoreBridge.fail_execution(execution.id, error_data, retry=False)


async def run_worker(queues: list[str], worker_id: Optional[str] = None):
    """Run a worker (main entry point)"""
    # Ensure core is initialized before starting worker
    # This initializes the database pool without running migrations
    try:
        CoreBridge.initialize(auto_migrate=False)
    except Exception as e:
        logger.warning(f"Failed to initialize Rust bridge: {e}")

    worker = Worker(queues, worker_id)

    try:
        await worker.start()
    except KeyboardInterrupt:
        logger.info("Received interrupt signal")
    finally:
        await worker.stop()

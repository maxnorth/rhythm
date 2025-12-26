"""Worker implementation for executing tasks and workflows"""

import asyncio
import logging
import signal
import time
import traceback

from rhythm.core import RhythmCore
from rhythm.registry import get_function

logger = logging.getLogger(__name__)


def _handle_shutdown_signal(signum, frame):
    """Signal handler for graceful shutdown"""
    signal_name = signal.Signals(signum).name
    logger.info(f"Received {signal_name} signal, requesting shutdown...")
    try:
        RhythmCore.request_shutdown()
    except Exception as e:
        logger.error(f"Error requesting shutdown: {e}")


def run():
    """Run a worker loop that polls for and executes tasks.

    The worker runs synchronously in a single thread, polling the database
    for pending tasks and executing them one at a time.

    Meta:
        section: Worker
    """
    logger.info("Worker starting...")

    # Register signal handlers for graceful shutdown
    signal.signal(signal.SIGINT, _handle_shutdown_signal)
    signal.signal(signal.SIGTERM, _handle_shutdown_signal)
    logger.debug("Signal handlers registered for SIGINT and SIGTERM")

    # Ensure core is initialized
    try:
        RhythmCore.initialize(auto_migrate=False)
    except Exception as e:
        logger.warning(f"Failed to initialize Rust adapter: {e}")

    # Start the internal worker (scheduler queue processor)
    RhythmCore.start_internal_worker()

    # Simple infinite loop
    while True:
        try:
            # Call Rust cooperative worker loop - returns quickly with an action
            logger.debug("Calling cooperative worker loop...")
            action = RhythmCore.run_cooperative_worker_loop()

            # Handle different action types
            if action.type == "execute_task":
                logger.info(
                    f"Received task: {action.target_name} (execution {action.execution_id})"
                )

                # Execute the task synchronously
                fn = get_function(action.target_name)

                # Only support sync functions
                if asyncio.iscoroutinefunction(fn):
                    raise TypeError(f"Async functions not supported: {action.target_name}")

                logger.debug(f"Executing sync function {action.target_name}")
                try:
                    result = fn(**action.inputs)
                except Exception as e:
                    logger.error(
                        f"Error executing {action.execution_id}: {e}\n{traceback.format_exc()}"
                    )

                    error_data = {
                        "message": str(e),
                        "type": type(e).__name__,
                        "traceback": traceback.format_exc(),
                    }

                    # Report the failure
                    RhythmCore.fail_execution(action.execution_id, error_data, retry=False)
                    continue

                # Mark as completed
                RhythmCore.complete_execution(action.execution_id, result)

            elif action.type == "continue":
                # Workflow was executed internally, check for more work immediately
                logger.debug("Continue action - checking for more work")
                continue

            elif action.type == "wait":
                # No work available, wait before checking again
                wait_seconds = action.duration_ms / 1000.0
                logger.debug(f"Wait action - sleeping for {wait_seconds}s")
                time.sleep(wait_seconds)

            elif action.type == "shutdown":
                # Shutdown requested
                logger.info("Shutdown requested, exiting gracefully...")
                break

            else:
                logger.warning(f"Unknown action type: {action.type}")

        except KeyboardInterrupt:
            logger.info("Received interrupt signal, stopping...")
            break
        except Exception as e:
            logger.error(f"Error in worker loop: {e}")
            # Continue on error

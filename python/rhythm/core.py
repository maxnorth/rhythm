"""Rhythm core interface"""

import json
from typing import Any, Dict, List, Optional

try:
    from rhythm import rhythm_core as rust
except ImportError:
    raise ImportError("rhythm_core Rust extension not found.")

from rhythm.models import DelegatedAction, Execution


class RhythmCore:
    """Rhythm core interface for managing executions and workflows"""

    @staticmethod
    def initialize(
        database_url: Optional[str] = None,
        config_path: Optional[str] = None,
        auto_migrate: bool = True,
        workflows: Optional[List[Dict[str, str]]] = None,
    ) -> None:
        """
        Initialize Rhythm with configuration options.

        Args:
            database_url: Database URL (overrides config file and env vars)
            config_path: Path to config file (overrides default search)
            auto_migrate: Whether to automatically run migrations if database is not initialized
            workflows: List of workflow files to register (each with name, source, file_path)
        """
        workflows_json = None
        if workflows:
            workflows_json = json.dumps(workflows)

        rust.initialize_sync(
            database_url=database_url,
            config_path=config_path,
            auto_migrate=auto_migrate,
            workflows_json=workflows_json,
        )

    @staticmethod
    def create_execution(
        exec_type: str,
        target_name: str,
        queue: str,
        inputs: Dict[str, Any],
        parent_workflow_id: Optional[str] = None,
    ) -> str:
        """Create a new execution"""
        return rust.create_execution_sync(
            exec_type=exec_type,
            target_name=target_name,
            queue=queue,
            inputs=json.dumps(inputs),
            parent_workflow_id=parent_workflow_id,
        )

    @staticmethod
    def run_cooperative_worker_loop() -> DelegatedAction:
        """
        Run cooperative worker loop - blocks until task needs host execution.

        This method runs an infinite loop in Rust that:
        - Claims work from the queue
        - Executes workflows internally
        - Returns tasks to the host for execution

        Only returns when it has a task that needs to be executed by the host.
        Queue is hardcoded to "default".

        Returns a DelegatedAction indicating what the host should do.
        """
        result = rust.run_cooperative_worker_loop()
        data = json.loads(result)
        return DelegatedAction.from_dict(data)

    @staticmethod
    def request_shutdown() -> None:
        """
        Request graceful shutdown of worker loops.

        Triggers the shutdown token, causing all active worker loops to
        exit gracefully on their next iteration (~100ms latency).
        """
        rust.request_shutdown()

    @staticmethod
    def start_internal_worker() -> None:
        """
        Start the internal worker (scheduler queue processor).

        This should be called when starting a worker process.
        Not intended for public API use.
        """
        rust.start_internal_worker()

    @staticmethod
    def complete_execution(execution_id: str, result: Any) -> None:
        """Complete an execution"""
        rust.complete_execution_sync(execution_id=execution_id, result=json.dumps(result))

    @staticmethod
    def fail_execution(execution_id: str, error: Dict[str, Any], retry: bool) -> None:
        """Fail an execution"""
        rust.fail_execution_sync(execution_id=execution_id, error=json.dumps(error), retry=retry)

    @staticmethod
    def get_execution(execution_id: str) -> Optional[Execution]:
        """Get execution by ID"""
        result = rust.get_execution_sync(execution_id=execution_id)
        if result:
            data = json.loads(result)
            return Execution.from_dict(data)
        return None

    @staticmethod
    def get_workflow_tasks(workflow_id: str) -> List[Dict[str, Any]]:
        """Get workflow child tasks"""
        result = rust.get_workflow_tasks_sync(workflow_id=workflow_id)
        return json.loads(result)

    @staticmethod
    def start_workflow(workflow_name: str, inputs: dict) -> str:
        """
        Start a workflow execution.

        Args:
            workflow_name: Name of the workflow to execute
            inputs: Input parameters for the workflow

        Returns:
            Workflow execution ID
        """
        inputs_json = json.dumps(inputs)
        return rust.start_workflow_sync(
            workflow_name=workflow_name,
            inputs_json=inputs_json,
        )

    @staticmethod
    def schedule_workflow(
        workflow_name: str,
        inputs: dict,
        run_at: str,
        queue: str = "default",
    ) -> str:
        """
        Schedule a workflow to start at a future time.

        Creates the execution immediately in Pending status, then schedules
        it to be enqueued at the specified time.

        Args:
            workflow_name: Name of the workflow to execute
            inputs: Input parameters for the workflow
            run_at: ISO 8601 datetime string (e.g., "2024-01-15T10:30:00")
            queue: Queue name (defaults to "default")

        Returns:
            Workflow execution ID
        """
        return rust.schedule_execution_sync(
            exec_type="workflow",
            target_name=workflow_name,
            inputs_json=json.dumps(inputs),
            run_at_iso=run_at,
            queue=queue,
        )

    @staticmethod
    def schedule_task(
        task_name: str,
        inputs: dict,
        run_at: str,
        queue: str = "default",
    ) -> str:
        """
        Schedule a task to start at a future time.

        Creates the execution immediately in Pending status, then schedules
        it to be enqueued at the specified time.

        Args:
            task_name: Name of the task to execute
            inputs: Input parameters for the task
            run_at: ISO 8601 datetime string (e.g., "2024-01-15T10:30:00")
            queue: Queue name (defaults to "default")

        Returns:
            Task execution ID
        """
        return rust.schedule_execution_sync(
            exec_type="task",
            target_name=task_name,
            inputs_json=json.dumps(inputs),
            run_at_iso=run_at,
            queue=queue,
        )

    @staticmethod
    def send_signal(
        workflow_id: str,
        signal_name: str,
        payload: Any,
        queue: str = "default",
    ) -> None:
        """
        Send a signal to a workflow.

        The workflow will be enqueued for processing and will pick up the
        signal on its next resumption.

        Args:
            workflow_id: ID of the workflow to send the signal to
            signal_name: Name of the signal channel
            payload: Signal payload (will be JSON serialized)
            queue: Queue name (defaults to "default")
        """
        rust.send_signal_sync(
            workflow_id=workflow_id,
            signal_name=signal_name,
            payload_json=json.dumps(payload),
            queue=queue,
        )

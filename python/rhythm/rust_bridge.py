"""Bridge to Rust core via PyO3"""

import json
from typing import Any, Dict, List, Optional

try:
    from rhythm import rhythm_core as rust
except ImportError:
    raise ImportError(
        "rhythm_core Rust extension not found."
    )


class RustBridge:
    """Wrapper around Rust core functions"""

    @staticmethod
    def initialize(
        database_url: Optional[str] = None,
        config_path: Optional[str] = None,
        auto_migrate: bool = True,
        require_initialized: bool = True,
        workflows: Optional[List[Dict[str, str]]] = None,
    ) -> None:
        """
        Initialize Rhythm with configuration options.

        Args:
            database_url: Database URL (overrides config file and env vars)
            config_path: Path to config file (overrides default search)
            auto_migrate: Whether to automatically run migrations if database is not initialized
            require_initialized: Whether to fail if database is not initialized (when auto_migrate is False)
            workflows: List of workflow files to register (each with name, source, file_path)
        """
        workflows_json = None
        if workflows:
            workflows_json = json.dumps(workflows)

        rust.initialize_sync(
            database_url=database_url,
            config_path=config_path,
            auto_migrate=auto_migrate,
            require_initialized=require_initialized,
            workflows_json=workflows_json,
        )

    @staticmethod
    def create_execution(
        exec_type: str,
        function_name: str,
        queue: str,
        inputs: Dict[str, Any],
        max_retries: int,
        parent_workflow_id: Optional[str] = None,
    ) -> str:
        """Create a new execution"""
        return rust.create_execution_sync(
            exec_type=exec_type,
            function_name=function_name,
            queue=queue,
            inputs=json.dumps(inputs),
            max_retries=max_retries,
            parent_workflow_id=parent_workflow_id,
        )

    @staticmethod
    def claim_execution(worker_id: str, queues: List[str]) -> Optional[Dict[str, Any]]:
        """Claim an execution for a worker"""
        result = rust.claim_execution_sync(worker_id=worker_id, queues=queues)
        if result:
            return json.loads(result)
        return None

    @staticmethod
    def complete_execution(execution_id: str, result: Any) -> None:
        """Complete an execution"""
        rust.complete_execution_sync(execution_id=execution_id, result=json.dumps(result))

    @staticmethod
    def fail_execution(execution_id: str, error: Dict[str, Any], retry: bool) -> None:
        """Fail an execution"""
        rust.fail_execution_sync(execution_id=execution_id, error=json.dumps(error), retry=retry)

    @staticmethod
    def get_execution(execution_id: str) -> Optional[Dict[str, Any]]:
        """Get execution by ID"""
        result = rust.get_execution_sync(execution_id=execution_id)
        if result:
            return json.loads(result)
        return None

    @staticmethod
    def get_workflow_tasks(workflow_id: str) -> List[Dict[str, Any]]:
        """Get workflow child tasks"""
        result = rust.get_workflow_tasks_sync(workflow_id=workflow_id)
        return json.loads(result)

    @staticmethod
    def migrate() -> None:
        """Run database migrations"""
        rust.migrate_sync()

    @staticmethod
    def run_cli(args: List[str]) -> None:
        """
        Run the CLI by calling into Rust.

        Args:
            args: Command-line arguments (sys.argv)

        The Rust code parses the provided arguments.
        This allows the CLI logic to live entirely in Rust while being invoked
        from Python.
        """
        rust.run_cli_sync(args)

    @staticmethod
    def run_benchmark(
        worker_command: List[str],
        workers: int,
        tasks: int,
        workflows: int,
        task_type: str,
        payload_size: int,
        tasks_per_workflow: int,
        queues: str,
        queue_distribution: Optional[str],
        duration: Optional[str],
        rate: Optional[float],
        compute_iterations: int,
        warmup_percent: float,
    ) -> None:
        """
        Run a benchmark by calling into Rust.

        Args:
            worker_command: Command to spawn workers (e.g., ["python", "-m", "rhythm", "worker"])
            workers: Number of worker processes to spawn
            tasks: Number of tasks to enqueue
            workflows: Number of workflows to enqueue
            task_type: Type of task ('noop' or 'compute')
            payload_size: Size of payload in bytes
            tasks_per_workflow: Number of tasks per workflow
            queues: Comma-separated queue names
            queue_distribution: Queue distribution percentages
            duration: Benchmark duration (e.g., "30s", "5m")
            rate: Task enqueue rate (tasks/sec)
            compute_iterations: Iterations for compute task type
            warmup_percent: Percentage of executions to exclude from metrics
        """
        rust.run_benchmark_sync(
            worker_command=worker_command,
            workers=workers,
            tasks=tasks,
            workflows=workflows,
            task_type=task_type,
            payload_size=payload_size,
            tasks_per_workflow=tasks_per_workflow,
            queues=queues,
            queue_distribution=queue_distribution,
            duration=duration,
            rate=rate,
            compute_iterations=compute_iterations,
            warmup_percent=warmup_percent,
        )

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


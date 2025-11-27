"""Bridge to core via PyO3"""

import json
from typing import Any, Dict, List, Optional

try:
    from rhythm import rhythm_core as rust
except ImportError:
    raise ImportError(
        "rhythm_core Rust extension not found."
    )

from rhythm.models import Execution


class CoreBridge:
    """Bridge to Rhythm core functionality"""

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
        function_name: str,
        queue: str,
        inputs: Dict[str, Any],
        parent_workflow_id: Optional[str] = None,
    ) -> str:
        """Create a new execution"""
        return rust.create_execution_sync(
            exec_type=exec_type,
            function_name=function_name,
            queue=queue,
            inputs=json.dumps(inputs),
            parent_workflow_id=parent_workflow_id,
        )

    @staticmethod
    def claim_execution(worker_id: str, queues: List[str]) -> Optional[Execution]:
        """Claim an execution for a worker"""
        result = rust.claim_execution_sync(worker_id=worker_id, queues=queues)
        if result:
            data = json.loads(result)
            return Execution.from_dict(data)
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


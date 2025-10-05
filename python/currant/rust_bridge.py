"""Bridge to Rust core via PyO3"""

import json
from typing import Any, Dict, List, Optional

try:
    import currant_core as rust
except ImportError:
    raise ImportError(
        "currant_core Rust extension not found. "
        "Build it with: cd core && maturin develop"
    )


class RustBridge:
    """Wrapper around Rust core functions"""

    @staticmethod
    def create_execution(
        exec_type: str,
        function_name: str,
        queue: str,
        priority: int,
        args: List[Any],
        kwargs: Dict[str, Any],
        max_retries: int,
        timeout_seconds: Optional[int],
        parent_workflow_id: Optional[str] = None,
    ) -> str:
        """Create a new execution"""
        return rust.create_execution_sync(
            exec_type=exec_type,
            function_name=function_name,
            queue=queue,
            priority=priority,
            args=json.dumps(args),
            kwargs=json.dumps(kwargs),
            max_retries=max_retries,
            timeout_seconds=timeout_seconds,
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
        rust.complete_execution_sync(
            execution_id=execution_id, result=json.dumps(result)
        )

    @staticmethod
    def fail_execution(execution_id: str, error: Dict[str, Any], retry: bool) -> None:
        """Fail an execution"""
        rust.fail_execution_sync(
            execution_id=execution_id, error=json.dumps(error), retry=retry
        )

    @staticmethod
    def suspend_workflow(workflow_id: str, checkpoint: Dict[str, Any]) -> None:
        """Suspend a workflow"""
        rust.suspend_workflow_sync(
            workflow_id=workflow_id, checkpoint=json.dumps(checkpoint)
        )

    @staticmethod
    def resume_workflow(workflow_id: str) -> None:
        """Resume a workflow"""
        rust.resume_workflow_sync(workflow_id=workflow_id)

    @staticmethod
    def get_execution(execution_id: str) -> Optional[Dict[str, Any]]:
        """Get execution by ID"""
        result = rust.get_execution_sync(execution_id=execution_id)
        if result:
            return json.loads(result)
        return None

    @staticmethod
    def get_workflow_activities(workflow_id: str) -> List[Dict[str, Any]]:
        """Get workflow activities"""
        result = rust.get_workflow_activities_sync(workflow_id=workflow_id)
        return json.loads(result)

    @staticmethod
    def update_heartbeat(worker_id: str, queues: List[str]) -> None:
        """Update worker heartbeat"""
        rust.update_heartbeat_sync(worker_id=worker_id, queues=queues)

    @staticmethod
    def stop_worker(worker_id: str) -> None:
        """Stop a worker"""
        rust.stop_worker_sync(worker_id=worker_id)

    @staticmethod
    def recover_dead_workers(timeout_seconds: int) -> int:
        """Recover dead workers"""
        return rust.recover_dead_workers_sync(timeout_seconds=timeout_seconds)

    @staticmethod
    def send_signal(
        workflow_id: str, signal_name: str, payload: Dict[str, Any]
    ) -> str:
        """Send a signal to a workflow"""
        return rust.send_signal_sync(
            workflow_id=workflow_id,
            signal_name=signal_name,
            payload=json.dumps(payload),
        )

    @staticmethod
    def get_signals(workflow_id: str, signal_name: str) -> List[Dict[str, Any]]:
        """Get signals for a workflow"""
        result = rust.get_signals_sync(workflow_id=workflow_id, signal_name=signal_name)
        return json.loads(result)

    @staticmethod
    def consume_signal(signal_id: str) -> None:
        """Consume a signal"""
        rust.consume_signal_sync(signal_id=signal_id)

    @staticmethod
    def migrate() -> None:
        """Run database migrations"""
        rust.migrate_sync()

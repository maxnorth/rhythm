"""
Comprehensive workflow test suite covering various DSL syntax and functionality.

## Test Coverage

### Currently Tested (12 tests passing):
1. **test_sequential_tasks**: Simple sequential execution of multiple tasks
2. **test_property_access**: Accessing nested properties from task results
3. **test_complex_expressions**: Multiple operations with property access
4. **test_object_construction**: Building objects from multiple task results
5. **test_no_tasks**: Workflow that returns immediately without any tasks
6. **test_single_task**: Simplest case - one task execution
7. **test_deeply_nested_properties**: Accessing deeply nested properties
8. **test_multiple_property_chains**: Multiple property accesses in single call
9. **test_literal_values**: Using literal numbers, strings, booleans
10. **test_mixed_inputs_and_results**: Mixing workflow inputs and task results
11. **test_empty_object**: Passing empty object {} to tasks
12. **test_return_literal**: Returning literal object without tasks

### Future Tests (Not Yet Implemented):
- Conditional logic (if/else)
- Variable reassignment (without 'let')
- Array operations
- Error handling (try/catch)
- Parallel execution (Promise.all)
- Loops (for/while)
"""

import subprocess
import sys
import time
import signal
import os
from pathlib import Path
import pytest
from rhythm.rust_bridge import RustBridge


# ============================================================================
# Fixtures
# ============================================================================

@pytest.fixture(scope="module")
def worker_process():
    """Start a worker process for the test session"""
    # Get workflow directory
    workflow_dir = Path(__file__).parent / "test_workflows"

    # Load all .flow files
    workflow_defs = []
    for flow_file in sorted(workflow_dir.glob("*.flow")):
        workflow_name = flow_file.stem
        workflow_source = flow_file.read_text()
        workflow_defs.append({
            "name": workflow_name,
            "source": workflow_source,
            "file_path": str(flow_file)
        })

    RustBridge.initialize(
        database_url="postgresql://rhythm@localhost/rhythm",
        auto_migrate=True,
        workflows=workflow_defs
    )

    # Start worker in background with fast polling for tests
    env = os.environ.copy()
    env['WORKFLOWS_WORKER_POLL_INTERVAL'] = '0.1'  # 100ms poll interval for tests

    process = subprocess.Popen(
        [
            sys.executable, "-m", "rhythm", "worker",
            "--queue", "default",
            "--queue", "system",
            "--import", "tests.test_workflows.tasks"
        ],
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True
    )

    yield process

    # Cleanup: stop worker
    process.send_signal(signal.SIGTERM)
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        process.kill()


def wait_for_workflow_completion(workflow_id, max_wait=30):
    """Helper to wait for workflow completion and return final status"""
    start_time = time.time()
    last_status = None
    poll_count = 0

    while time.time() - start_time < max_wait:
        poll_start = time.time()
        workflow = RustBridge.get_execution(workflow_id)
        poll_time = time.time() - poll_start

        if not workflow:
            raise Exception(f"Workflow {workflow_id} not found!")

        status = workflow['status']
        poll_count += 1

        # Log status changes
        if status != last_status:
            elapsed = time.time() - start_time
            print(f"  [{elapsed:.3f}s] Status: {last_status} -> {status} (poll #{poll_count}, query took {poll_time*1000:.1f}ms)")
            last_status = status

        if status == 'completed':
            total_time = time.time() - start_time
            print(f"  [DONE] Completed in {total_time:.3f}s after {poll_count} polls")
            return workflow
        elif status == 'failed':
            error = workflow.get('error', 'Unknown error')
            raise Exception(f"Workflow failed: {error}")

        time.sleep(0.01)  # 10ms poll interval

    final_workflow = RustBridge.get_execution(workflow_id)
    raise Exception(f"Workflow did not complete within {max_wait}s. Final status: {final_workflow['status']}")


# ============================================================================
# Tests
# ============================================================================

def test_sequential_tasks(worker_process):
    """Test simple sequential execution of multiple tasks"""
    print("\n=== test_sequential_tasks ===")
    start = time.time()

    start_wf = time.time()
    workflow_id = RustBridge.start_workflow("sequential_tasks", {"start": 0})
    print(f"  [START] Workflow {workflow_id} created in {(time.time() - start_wf)*1000:.1f}ms")

    result = wait_for_workflow_completion(workflow_id)

    assert result['result'] == {"result": 3}

    # Verify 3 tasks were executed
    child_tasks = RustBridge.get_workflow_tasks(workflow_id)
    work_tasks = [t for t in child_tasks if t['function_name'] == 'increment']
    assert len(work_tasks) == 3

    print(f"  [TOTAL] Test took {time.time() - start:.3f}s\n")


def test_property_access(worker_process):
    """Test accessing nested properties from task results"""
    workflow_id = RustBridge.start_workflow("property_access", {"name": "Alice", "age": 30})
    result = wait_for_workflow_completion(workflow_id)

    assert result['result'] == {"greeting": "Hello Alice, age 30!"}


def test_complex_expressions(worker_process):
    """Test multiple operations with property access and arithmetic"""
    workflow_id = RustBridge.start_workflow("complex_expressions", {})
    result = wait_for_workflow_completion(workflow_id)

    # (5 + 3) * 2 = 16
    assert result['result'] == {"result": 16}


def test_object_construction(worker_process):
    """Test building objects from multiple task results"""
    workflow_id = RustBridge.start_workflow("object_construction", {
        "user_id": "123",
        "title": "Dr."
    })
    result = wait_for_workflow_completion(workflow_id)

    assert result['result'] == {"full_name": "Dr. John Doe"}


def test_no_tasks(worker_process):
    """Test workflow that returns immediately without executing any tasks"""
    workflow_id = RustBridge.start_workflow("no_tasks", {"value": 42})
    result = wait_for_workflow_completion(workflow_id)

    assert result['result'] == {"message": "No tasks executed", "input": 42}


def test_single_task(worker_process):
    """Test simplest case - one task execution"""
    workflow_id = RustBridge.start_workflow("single_task", {"message": "Hello World"})
    result = wait_for_workflow_completion(workflow_id)

    assert result['result'] == {"message": "Hello World"}


def test_deeply_nested_properties(worker_process):
    """Test accessing deeply nested properties (e.g., data.level1.level2.level3.value)"""
    workflow_id = RustBridge.start_workflow("deeply_nested_properties", {})
    result = wait_for_workflow_completion(workflow_id)

    # 42 * 2 = 84
    assert result['result'] == {"processed": 84}


def test_multiple_property_chains(worker_process):
    """Test multiple property accesses in single task call"""
    workflow_id = RustBridge.start_workflow("multiple_property_chains", {})
    result = wait_for_workflow_completion(workflow_id)

    assert result['result'] == {"combined": "Bob (age 25) - v1.0.0 @ 1234567890"}


def test_literal_values(worker_process):
    """Test using literal numbers, strings, booleans in task arguments"""
    workflow_id = RustBridge.start_workflow("literal_values", {})
    result = wait_for_workflow_completion(workflow_id)

    assert result['result'] == {
        "record": {
            "id": 123,
            "name": "test",
            "active": True,
            "score": 99.5
        }
    }


def test_mixed_inputs_and_results(worker_process):
    """Test mixing workflow inputs and task results in same call"""
    workflow_id = RustBridge.start_workflow("mixed_inputs_and_results", {
        "start": 10,
        "offset": 5
    })
    result = wait_for_workflow_completion(workflow_id)

    # step1 = 11, step2 = 10 + 11 = 21, step3 = 21 + 5 = 26
    assert result['result'] == {"result": 26}


def test_empty_object(worker_process):
    """Test passing empty object {} to tasks"""
    workflow_id = RustBridge.start_workflow("empty_object", {})
    result = wait_for_workflow_completion(workflow_id)

    assert result['result'] == {"defaults": {"timeout": 30, "retries": 3}}


def test_return_literal(worker_process):
    """Test returning literal object without any task execution"""
    workflow_id = RustBridge.start_workflow("return_literal", {})
    result = wait_for_workflow_completion(workflow_id)

    assert result['result'] == {"status": "success", "code": 200}

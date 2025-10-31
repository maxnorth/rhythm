#!/usr/bin/env python3
"""
Manual test script for workflow execution.

This script:
1. Finds a pending workflow
2. Manually executes workflow steps to test the interpreter
3. Shows state changes after each step
"""

import asyncio
import asyncpg
import sys
sys.path.insert(0, '/Users/maxnorth/Projects/rhythm/python')

import rhythm
rust = rhythm.rhythm_core


async def main():
    conn = await asyncpg.connect('postgresql://rhythm@localhost/rhythm')

    print("=== Checking initial workflow ===")

    # Get the latest pending workflow
    workflow = await conn.fetchrow("""
        SELECT id, function_name, status
        FROM executions
        WHERE type = 'workflow' AND status = 'pending'
        ORDER BY created_at DESC
        LIMIT 1
    """)

    if not workflow:
        print("No pending workflows found. Please run main.py first to create one.")
        await conn.close()
        return

    workflow_id = workflow['id']
    print(f"Found workflow: {workflow_id} ({workflow['function_name']})")

    # Check context
    context = await conn.fetchrow("""
        SELECT statement_index, locals, awaiting_task_id
        FROM workflow_execution_context
        WHERE execution_id = $1
    """, workflow_id)

    print(f"  Statement index: {context['statement_index']}")
    print(f"  Awaiting task: {context['awaiting_task_id']}")

    # Execute first step
    print("\n=== Executing first workflow step ===")
    try:
        result = rust.execute_workflow_step_sync(execution_id=workflow_id)
        print(f"Result: {result}")
    except Exception as e:
        print(f"Error: {e}")

    # Check state after execution
    workflow = await conn.fetchrow("""
        SELECT id, function_name, status
        FROM executions
        WHERE id = $1
    """, workflow_id)

    context = await conn.fetchrow("""
        SELECT statement_index, locals, awaiting_task_id
        FROM workflow_execution_context
        WHERE execution_id = $1
    """, workflow_id)

    print(f"\nAfter execution:")
    print(f"  Status: {workflow['status']}")
    print(f"  Statement index: {context['statement_index']}")
    print(f"  Awaiting task: {context['awaiting_task_id']}")

    # Check if any child tasks were created
    child_tasks = await conn.fetch("""
        SELECT id, function_name, status, kwargs
        FROM executions
        WHERE parent_workflow_id = $1
        ORDER BY created_at DESC
    """, workflow_id)

    if child_tasks:
        print(f"\nChild tasks created:")
        for task in child_tasks:
            print(f"  - {task['id']}: {task['function_name']} ({task['status']})")
            print(f"    Inputs: {task['kwargs']}")

    await conn.close()


if __name__ == "__main__":
    asyncio.run(main())

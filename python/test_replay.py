"""Test is_replaying() functionality"""

import asyncio
from currant import workflow, task, is_replaying


@task(queue="test", retries=2)
async def step1():
    """First task"""
    print("[TASK] Step 1 executing")
    return {"step": 1, "result": "done"}


@task(queue="test", retries=2)
async def step2():
    """Second task"""
    print("[TASK] Step 2 executing")
    return {"step": 2, "result": "done"}


@task(queue="test", retries=2)
async def step3():
    """Third task"""
    print("[TASK] Step 3 executing")
    return {"step": 3, "result": "done"}


@workflow(queue="test", version=1)
async def test_workflow():
    """Test workflow to verify is_replaying() works correctly"""

    if not is_replaying():
        print("\n[WORKFLOW] ========== Starting workflow ==========")
    else:
        print("\n[WORKFLOW] (replaying - this should not print)")

    # Step 1
    result1 = await step1.run()
    if not is_replaying():
        print(f"[WORKFLOW] Step 1 completed: {result1}")
    else:
        print(f"[WORKFLOW] (replaying step 1 - this should not print)")

    # Step 2
    result2 = await step2.run()
    if not is_replaying():
        print(f"[WORKFLOW] Step 2 completed: {result2}")
    else:
        print(f"[WORKFLOW] (replaying step 2 - this should not print)")

    # Step 3
    result3 = await step3.run()
    if not is_replaying():
        print(f"[WORKFLOW] Step 3 completed: {result3}")
    else:
        print(f"[WORKFLOW] (replaying step 3 - this should not print)")

    if not is_replaying():
        print("[WORKFLOW] ========== Workflow complete ==========\n")
    else:
        print("[WORKFLOW] (replaying completion - this should not print)")

    return {
        "status": "completed",
        "results": [result1, result2, result3]
    }


# Separate enqueue script
if __name__ == "__main__":
    # Import the module to register functions with correct name
    import test_replay
    
    async def enqueue():
        print("Enqueueing test workflow...")
        workflow_id = await test_replay.test_workflow.queue()
        print(f"✓ Workflow enqueued: {workflow_id}")
        print("\nRun worker with:")
        print(f"  python -m currant worker -q test -m test_replay")
    
    asyncio.run(enqueue())

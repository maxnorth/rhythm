#!/usr/bin/env python3
"""Manual test script for workflows"""

import asyncio
import sys
from examples.simple_example import send_notification, process_order_workflow


async def main():
    print("=" * 60)
    print("Manual Test - Enqueue Tasks and Workflows")
    print("=" * 60)
    print()

    # Enqueue a simple task
    print("1. Enqueuing notification task...")
    try:
        task_id = await send_notification.queue(
            user_id="test_user_123",
            message="Test notification from manual test"
        )
        print(f"   ✓ Task enqueued: {task_id}")
    except Exception as e:
        print(f"   ✗ Failed: {e}")
        sys.exit(1)

    print()
    print("2. Enqueuing workflow...")
    try:
        workflow_id = await process_order_workflow.queue(
            order_id="test_order_456",
            customer_email="test@example.com",
            amount=100,
            payment_method="credit_card",
            items=["item1", "item2"]
        )
        print(f"   ✓ Workflow enqueued: {workflow_id}")
    except Exception as e:
        print(f"   ✗ Failed: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)

    print()
    print("=" * 60)
    print("Success! Tasks and workflows enqueued.")
    print("=" * 60)
    print()
    print("Now start a worker in another terminal:")
    print("  currant worker -q notifications -q orders -m examples.simple_example")
    print()


if __name__ == "__main__":
    asyncio.run(main())

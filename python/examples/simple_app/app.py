#!/usr/bin/env python3
"""
App example - demonstrates how to schedule workflows and tasks.

Run this after starting the worker to see tasks get executed.
"""

import os

import rhythm


def main():
    """Main entry point - demonstrates scheduling workflows and tasks"""

    # Initialize Rhythm client (no migration needed for scheduler)
    database_url = os.environ.get(
        "RHYTHM_DATABASE_URL", "postgresql://rhythm@localhost/rhythm"
    )

    print("Initializing Rhythm client...")
    rhythm.init(
        database_url=database_url
    )

    # Step 1: Schedule all work
    print("\n=== Scheduling work ===")

    # Schedule workflow
    workflow_id = rhythm.client.queue_workflow(
        name="simple_test",
        inputs={
            "orderId": "order-12345",
        },
    )
    print(f"✓ Workflow 'simple_test' scheduled: {workflow_id}")

    # Schedule standalone tasks
    email_id = rhythm.client.queue_task(
        name="send_email",
        inputs={
            "to": "admin@example.com",
            "subject": "System Alert",
            "body": "This is a test email from Rhythm",
        },
    )
    print(f"✓ Email task queued: {email_id}")

    notification_id = rhythm.client.queue_task(
        name="send_notification",
        inputs={
            "user_id": "user-123",
            "message": "Hello from Rhythm!",
        },
    )
    print(f"✓ Notification task queued: {notification_id}")

    # Step 2: Wait for all work to complete
    print("\n=== Waiting for results ===")

    print("Waiting for workflow to complete...")
    workflow_result = rhythm.client.wait_for_execution(workflow_id, timeout=30.0)
    print(f"✓ Workflow completed with status: {workflow_result.status}")
    if workflow_result.status == "completed":
        print(f"  Output: {workflow_result.output}")
    else:
        print(f"  Workflow failed or was cancelled")

    print("\nWaiting for tasks to complete...")
    email_result = rhythm.client.wait_for_execution(email_id, timeout=10.0)
    print(f"✓ Email task completed: {email_result.output}")

    notification_result = rhythm.client.wait_for_execution(notification_id, timeout=10.0)
    print(f"✓ Notification task completed: {notification_result.output}")

    print("\n✓ All work completed successfully!")


if __name__ == "__main__":
    main()

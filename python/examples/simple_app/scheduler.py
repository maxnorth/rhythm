#!/usr/bin/env python3
"""
Scheduler entry point for the simple application.

Enqueues workflows and tasks for processing.
"""

import asyncio
import logging
import os
import sys

# Add rhythm to path (for development)
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "../.."))
from rhythm.client import queue_execution

import rhythm

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(name)s: %(message)s",
)

logger = logging.getLogger(__name__)


def schedule_workflow():
    """Schedule a workflow execution"""
    logger.info("Scheduling order processing workflow...")

    rhythm.start_workflow(
        "simple_test",
        inputs={
            "orderId": "order-12345",
        },
    )

    logger.info(f"Workflow scheduled")


def schedule_standalone_tasks():
    """Schedule standalone tasks (not part of a workflow)"""

    logger.info("Scheduling standalone tasks...")

    # Schedule an email task
    email_id = queue_execution(
        exec_type="task",
        function_name="send_email",
        inputs={
            "to": "admin@example.com",
            "subject": "System Alert",
            "body": "This is a test email from Rhythm",
        },
        queue="default",
    )
    logger.info(f"Email task scheduled with ID: {email_id}")

    # Schedule a notification task
    notification_id = queue_execution(
        exec_type="task",
        function_name="send_notification",
        inputs={
            "user_id": "user-123",
            "message": "Hello from Rhythm!",
        },
        queue="default",
    )
    logger.info(f"Notification task scheduled with ID: {notification_id}")

    return [email_id, notification_id]


def main():
    """Main entry point for the scheduler"""
    # Get database URL from environment
    database_url = os.environ.get(
        "RHYTHM_DATABASE_URL", "postgresql://rhythm@localhost/rhythm"
    )

    # Initialize Rhythm (no migration or workflow registration needed)
    logger.info("Initializing Rhythm...")
    rhythm.init(
        database_url=database_url,
        auto_migrate=False,
    )
    logger.info("Rhythm initialized")

    # Show menu
    print("\n=== Rhythm Example Scheduler ===")
    print("1. Schedule a workflow (process_order)")
    print("2. Schedule standalone tasks")
    print("3. Schedule both")
    print("4. Exit")

    choice = input("\nEnter your choice (1-4): ").strip()

    if choice == "1":
        schedule_workflow()
    elif choice == "2":
        schedule_standalone_tasks()
    elif choice == "3":
        schedule_workflow()
        schedule_standalone_tasks()
    elif choice == "4":
        logger.info("Exiting...")
        return
    else:
        logger.error("Invalid choice")
        return

    logger.info("Done! Check the worker logs to see task execution.")


if __name__ == "__main__":
    main()

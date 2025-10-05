"""Enqueue example jobs from imported module"""

import asyncio
from examples.simple_example import send_notification, process_order_workflow


async def main():
    """Enqueue some work"""
    print("=" * 60)
    print("Enqueuing example jobs and workflows")
    print("=" * 60 + "\n")

    # Enqueue a simple notification job
    job_id = await send_notification.queue(
        user_id="user_123",
        message="Your order has been confirmed!"
    )
    print(f"✓ Notification job enqueued: {job_id}\n")

    # Enqueue an order processing workflow
    workflow_id = await process_order_workflow.queue(
        order_id="order_456",
        customer_email="customer@example.com",
        amount=9999,
        payment_method="credit_card",
        items=["item1", "item2", "item3"],
    )
    print(f"✓ Order workflow enqueued: {workflow_id}\n")

    # Enqueue another order with high priority
    workflow_id_2 = await process_order_workflow.options(priority=10).queue(
        order_id="order_789",
        customer_email="vip@example.com",
        amount=19999,
        payment_method="credit_card",
        items=["premium_item"],
    )
    print(f"✓ High-priority order workflow enqueued: {workflow_id_2}\n")

    print("=" * 60)
    print("Jobs enqueued! Start workers to process them:")
    print("  workflows worker -q notifications -q orders -m examples.simple_example")
    print("=" * 60)


if __name__ == "__main__":
    asyncio.run(main())

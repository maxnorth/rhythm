"""
Simple example demonstrating jobs, activities, and workflows
"""

import asyncio
from workflows import job, activity, workflow, is_replaying


# Simple job that runs independently
@job(queue="notifications", retries=3)
async def send_notification(user_id: str, message: str):
    """Send a notification to a user"""
    print(f"[NOTIFICATION] Sending to user {user_id}: {message}")
    await asyncio.sleep(0.5)  # Simulate API call
    return {"sent": True, "user_id": user_id}


# Activities that are called from workflows
@activity(retries=3, timeout=60)
async def validate_order(order_id: str, amount: int):
    """Validate an order"""
    print(f"[VALIDATE] Validating order {order_id} for ${amount}")
    await asyncio.sleep(0.3)

    if amount < 0:
        raise ValueError("Amount must be positive")

    return {"valid": True, "order_id": order_id}


@activity(retries=5, timeout=120)
async def charge_payment(order_id: str, amount: int, payment_method: str):
    """Charge the payment"""
    print(f"[CHARGE] Charging ${amount} via {payment_method} for order {order_id}")
    await asyncio.sleep(0.5)

    # Simulate payment processing
    transaction_id = f"txn_{order_id}_{payment_method}"

    return {
        "success": True,
        "transaction_id": transaction_id,
        "amount": amount,
    }


@activity()
async def send_confirmation_email(email: str, order_id: str, amount: int):
    """Send order confirmation email"""
    print(f"[EMAIL] Sending confirmation to {email} for order {order_id} (${amount})")
    await asyncio.sleep(0.2)
    return {"sent": True, "email": email}


@activity()
async def update_inventory(order_id: str, items: list):
    """Update inventory after order"""
    print(f"[INVENTORY] Updating inventory for order {order_id}: {len(items)} items")
    await asyncio.sleep(0.3)
    return {"updated": True, "item_count": len(items)}


# Workflow that orchestrates the order processing
@workflow(queue="orders", version=1, timeout=600)
async def process_order_workflow(
    order_id: str,
    customer_email: str,
    amount: int,
    payment_method: str,
    items: list,
):
    """
    Process an order end-to-end with automatic retry and recovery.

    This workflow will survive crashes and resume from checkpoints.
    """
    if not is_replaying():
        print(f"\n[WORKFLOW] Starting order processing for {order_id}\n")

    # Step 1: Validate the order
    validation_result = await validate_order.run(order_id, amount)
    if not is_replaying():
        print(f"[WORKFLOW] ✓ Validation completed: {validation_result}\n")

    # Step 2: Charge the payment
    payment_result = await charge_payment.run(order_id, amount, payment_method)
    if not is_replaying():
        print(f"[WORKFLOW] ✓ Payment charged: {payment_result['transaction_id']}\n")

    # Step 3: Send confirmation email
    email_result = await send_confirmation_email.run(customer_email, order_id, amount)
    if not is_replaying():
        print(f"[WORKFLOW] ✓ Email sent: {email_result}\n")

    # Step 4: Update inventory
    inventory_result = await update_inventory.run(order_id, items)
    if not is_replaying():
        print(f"[WORKFLOW] ✓ Inventory updated: {inventory_result}\n")

    return {
        "status": "completed",
        "order_id": order_id,
        "transaction_id": payment_result["transaction_id"],
        "amount": amount,
    }


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
    print("  workflows worker -q notifications")
    print("  workflows worker -q orders")
    print("=" * 60)


if __name__ == "__main__":
    asyncio.run(main())

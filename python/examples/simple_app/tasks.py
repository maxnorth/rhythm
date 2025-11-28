"""Example tasks for the simple application"""

import asyncio
import logging
from rhythm import task
import time

logger = logging.getLogger(__name__)


@task
def send_email(to: str, subject: str, body: str) -> dict:
    """Simulate sending an email"""
    logger.info(f"Sending email to {to}: {subject}")
    time.sleep(2)
    logger.info(f"Email sent to {to}")
    return {"status": "sent", "to": to, "subject": subject}


@task
def process_payment(order_id: str, amount: float) -> dict:
    """Simulate processing a payment (sync function)"""
    logger.info(f"Processing payment for order {order_id}: ${amount}")
    # Simulate payment processing
    time.sleep(2)
    logger.info(f"Payment processed for order {order_id}")
    return {"status": "completed", "order_id": order_id, "amount": amount}


@task
def update_inventory(product_id: str, quantity: int) -> dict:
    """Update inventory for a product"""
    logger.info(f"Updating inventory for product {product_id}: {quantity} units")
    time.sleep(0.5)
    logger.info(f"Inventory updated for product {product_id}")
    return {"product_id": product_id, "new_quantity": quantity}


@task
def send_notification(user_id: str, message: str) -> dict:
    """Send a notification to a user"""
    logger.info(f"Sending notification to user {user_id}: {message}")
    time.sleep(0.3)
    logger.info(f"Notification sent to user {user_id}")
    return {"user_id": user_id, "delivered": True}

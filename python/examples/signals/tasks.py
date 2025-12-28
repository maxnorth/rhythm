"""Tasks for the signals example"""

import logging
import time

import rhythm

logger = logging.getLogger(__name__)


@rhythm.task
def validate_order(order_id: str, amount: float) -> dict:
    """Validate an order before approval"""
    logger.info(f"Validating order {order_id} for ${amount}")
    time.sleep(0.3)

    # Simple validation: reject orders over $10000
    if amount > 10000:
        logger.warning(f"Order {order_id} rejected: amount too high")
        return {"valid": False, "reason": "Amount exceeds maximum limit"}

    logger.info(f"Order {order_id} validated successfully")
    return {"valid": True}


@rhythm.task
def process_payment(order_id: str, amount: float) -> dict:
    """Process payment for an approved order"""
    logger.info(f"Processing payment for order {order_id}: ${amount}")
    time.sleep(0.5)
    logger.info(f"Payment processed for order {order_id}")
    return {"status": "completed", "order_id": order_id, "amount": amount}


@rhythm.task
def send_notification(user_id: str, message: str) -> dict:
    """Send a notification to a user"""
    logger.info(f"Sending notification to user {user_id}: {message}")
    time.sleep(0.2)
    logger.info(f"Notification sent to user {user_id}")
    return {"user_id": user_id, "delivered": True}

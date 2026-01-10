"""Tasks for the sub-workflows example.

These tasks are called by the workflows to perform actual business logic.
In a real application, these would integrate with payment gateways,
inventory systems, shipping providers, etc.
"""

import logging
import time
import uuid

import rhythm

logger = logging.getLogger(__name__)


# ==================== Email Tasks ====================

@rhythm.task
def send_email(to: str, subject: str, body: str) -> dict:
    """Send an email notification"""
    logger.info(f"Sending email to {to}: {subject}")
    time.sleep(0.2)  # Simulate email sending
    return {"sent": True, "to": to, "subject": subject}


# ==================== Payment Tasks ====================

@rhythm.task
def validate_payment_method(payment_method: str) -> dict:
    """Validate that a payment method is acceptable"""
    logger.info(f"Validating payment method: {payment_method}")
    valid_methods = ["credit_card", "debit_card", "paypal", "bank_transfer"]
    return {"valid": payment_method in valid_methods}


@rhythm.task
def charge_payment(order_id: str, amount: float, payment_method: str) -> dict:
    """Charge the customer's payment method"""
    logger.info(f"Charging ${amount} for order {order_id} via {payment_method}")
    time.sleep(0.5)  # Simulate payment processing

    # Simulate occasional payment failures
    if amount > 10000:
        return {"success": False, "error": "amount_exceeds_limit"}

    transaction_id = f"txn_{uuid.uuid4().hex[:12]}"
    logger.info(f"Payment successful: {transaction_id}")
    return {"success": True, "transaction_id": transaction_id}


@rhythm.task
def record_transaction(order_id: str, transaction_id: str, amount: float) -> dict:
    """Record a payment transaction in the ledger"""
    logger.info(f"Recording transaction {transaction_id} for order {order_id}")
    return {"recorded": True}


@rhythm.task
def refund_payment(order_id: str, transaction_id: str) -> dict:
    """Refund a payment"""
    logger.info(f"Refunding transaction {transaction_id} for order {order_id}")
    time.sleep(0.3)
    return {"refunded": True, "refund_id": f"ref_{uuid.uuid4().hex[:8]}"}


# ==================== Inventory Tasks ====================

@rhythm.task
def check_inventory(product_id: str, quantity: int) -> dict:
    """Check if a product has sufficient inventory"""
    logger.info(f"Checking inventory for {product_id}, quantity: {quantity}")
    # Simulate inventory check - product-999 is always out of stock
    available = product_id != "product-999"
    stock = 100 if available else 0
    return {"available": available and stock >= quantity, "stock": stock}


@rhythm.task
def reserve_item(order_id: str, product_id: str, quantity: int) -> dict:
    """Reserve inventory for an order"""
    logger.info(f"Reserving {quantity}x {product_id} for order {order_id}")
    return {"reserved": True, "reservation_id": f"res_{uuid.uuid4().hex[:8]}"}


@rhythm.task
def release_reservation(order_id: str, product_id: str) -> dict:
    """Release an inventory reservation"""
    logger.info(f"Releasing reservation for {product_id} on order {order_id}")
    return {"released": True}


# ==================== Shipping Tasks ====================

@rhythm.task
def get_customer_address(customer_email: str) -> dict:
    """Get customer's shipping address"""
    logger.info(f"Getting address for {customer_email}")
    # Simulate address lookup
    return {
        "street": "123 Main St",
        "city": "San Francisco",
        "state": "CA",
        "zip": "94102",
        "country": "US"
    }


@rhythm.task
def calculate_shipping(items: list, destination: dict) -> dict:
    """Calculate shipping cost and method"""
    logger.info(f"Calculating shipping to {destination['city']}, {destination['state']}")
    # Simulate shipping calculation
    total_items = sum(item.get("quantity", 1) for item in items)
    return {
        "method": "standard" if total_items < 5 else "express",
        "cost": 5.99 if total_items < 5 else 12.99,
        "estimated_days": 5 if total_items < 5 else 2
    }


@rhythm.task
def create_shipment(order_id: str, items: list, address: dict, shipping_method: str) -> dict:
    """Create a shipment with the carrier"""
    logger.info(f"Creating {shipping_method} shipment for order {order_id}")
    time.sleep(0.3)
    shipment_id = f"ship_{uuid.uuid4().hex[:10]}"
    tracking_number = f"TRK{uuid.uuid4().hex[:12].upper()}"
    return {
        "shipment_id": shipment_id,
        "tracking_number": tracking_number,
        "carrier": "FastShip"
    }


@rhythm.task
def schedule_pickup(shipment_id: str) -> dict:
    """Schedule carrier pickup for a shipment"""
    logger.info(f"Scheduling pickup for shipment {shipment_id}")
    return {"pickup_scheduled": True, "pickup_date": "2024-01-15"}

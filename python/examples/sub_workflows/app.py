#!/usr/bin/env python3
"""
Sub-Workflows Example - demonstrates parent and child workflows.

This example shows how to:
1. Create a parent workflow that orchestrates multiple child workflows
2. Pass data between parent and child workflows
3. Handle results from child workflows

The order fulfillment workflow:
- Calls process_payment workflow to handle payment
- Calls reserve_inventory workflow to reserve items
- Calls arrange_shipping workflow to set up delivery
- Sends confirmation email to customer

Run this after starting the worker:
    python worker.py  # In one terminal
    python app.py     # In another terminal
"""

import os
import rhythm


def main():
    """Demonstrate sub-workflows with an order fulfillment example"""

    database_url = os.environ.get(
        "RHYTHM_DATABASE_URL", "postgresql://rhythm@localhost/rhythm"
    )

    print("Initializing Rhythm client...")
    rhythm.init(database_url=database_url)

    # Create a sample order
    order = {
        "order_id": "ORD-2024-001",
        "customer_email": "customer@example.com",
        "items": [
            {"product_id": "product-123", "name": "Widget", "price": 29.99, "quantity": 2},
            {"product_id": "product-456", "name": "Gadget", "price": 49.99, "quantity": 1},
        ],
        "payment_method": "credit_card"
    }

    print("\n" + "=" * 60)
    print("SUB-WORKFLOWS EXAMPLE: Order Fulfillment")
    print("=" * 60)
    print(f"\nOrder ID: {order['order_id']}")
    print(f"Customer: {order['customer_email']}")
    print(f"Items: {len(order['items'])} items")
    total = sum(item['price'] * item['quantity'] for item in order['items'])
    print(f"Total: ${total:.2f}")
    print(f"Payment: {order['payment_method']}")

    print("\n--- Scheduling order_fulfillment workflow ---")
    print("This will spawn child workflows:")
    print("  1. process_payment - handle payment processing")
    print("  2. reserve_inventory - reserve items in warehouse")
    print("  3. arrange_shipping - set up delivery")

    # Queue the parent workflow
    workflow_id = rhythm.client.queue_workflow(
        name="order_fulfillment",
        inputs=order,
    )
    print(f"\nWorkflow queued: {workflow_id}")

    print("\n--- Waiting for workflow to complete ---")
    print("(The worker will process parent and child workflows)")

    # Wait for the workflow to complete
    result = rhythm.client.wait_for_execution(workflow_id, timeout=60.0)

    print("\n" + "=" * 60)
    print("RESULT")
    print("=" * 60)
    print(f"Status: {result.status}")

    if result.status == "completed":
        output = result.output
        print(f"\nOrder Status: {output.get('status')}")
        if output.get('status') == 'completed':
            print(f"Transaction ID: {output.get('payment_transaction_id')}")
            print(f"Tracking Number: {output.get('tracking_number')}")
            print(f"Total Charged: ${output.get('total'):.2f}")
        else:
            print(f"Failure Reason: {output.get('reason')}")
    else:
        print(f"Workflow failed with status: {result.status}")
        if result.output:
            print(f"Output: {result.output}")

    print("\n" + "=" * 60)
    print("Demo complete!")
    print("=" * 60)


if __name__ == "__main__":
    main()

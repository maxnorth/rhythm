"""
Example Python application using Currant workflows

Project structure:
workflow_example/
  workflows/
    processOrder.flow
    sendDailyReport.flow
  main.py
"""

import currant
import asyncio
import logging

logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(name)s - %(levelname)s - %(message)s')

# Initialize Currant with workflow paths
currant.init(
    database_url="postgresql://currant@localhost/currant",
    workflow_paths=[
        "./workflows",           # Main workflows directory
    ]
)


# Define tasks (these are called from workflows)
@currant.task(queue="default")
async def chargeCard(orderId: str, amount: float):
    """Charge customer's payment method"""
    print(f"💳 Charging ${amount} for order {orderId}")
    await asyncio.sleep(1)  # Simulate processing
    return {"success": True, "transaction_id": "tx_123456", "amount": amount}


@currant.task(queue="default")
async def shipOrder(orderId: str):
    """Ship the order"""
    print(f"📦 Shipping order {orderId}")
    await asyncio.sleep(1)  # Simulate shipping
    return {"success": True, "tracking_number": "TRACK123", "carrier": "UPS"}


@currant.task(queue="default")
async def sendEmail(to: str, subject: str, body: str):
    """Send email notification"""
    print(f"📧 Sending email to {to}: {subject}")
    await asyncio.sleep(0.5)  # Simulate email sending
    return {"success": True, "sent_at": "2025-10-19T12:00:00Z"}


# Example: Start a workflow and run a worker
async def main():
    print("="*60)
    print("Starting workflow...")
    print("="*60)

    # Queue a workflow execution
    workflow_id = await currant.start_workflow(
        "processOrder",
        inputs={
            "orderId": "order-123",
            "customerId": "cust-456",
            "amount": 99.99
        }
    )

    print(f"✅ Started workflow: {workflow_id}\n")

    print("="*60)
    print("Starting worker to execute workflow and tasks...")
    print("="*60)

    # Start a worker to execute the workflow
    from currant.worker import Worker

    worker = Worker(queues=["default"])

    try:
        # Run worker for a limited time to execute the workflow
        await asyncio.wait_for(worker.start(), timeout=30)
    except asyncio.TimeoutError:
        print("\n" + "="*60)
        print("Worker stopped after timeout")
        print("="*60)
        await worker.stop()


if __name__ == "__main__":
    asyncio.run(main())

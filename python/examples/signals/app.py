#!/usr/bin/env python3
"""
Start an approval workflow and wait for it to complete.

The workflow will pause waiting for an approval signal. Use send_approval.py
to approve or reject the workflow.

Usage:
    1. Start the worker:     python worker.py
    2. Run this app:         python app.py
    3. Send approval:        python send_approval.py <workflow_id>
       Or reject:            python send_approval.py <workflow_id> --reject
"""

import argparse
import os

import rhythm


def main():
    parser = argparse.ArgumentParser(description="Start an approval workflow")
    parser.add_argument("--order-id", default="order-001", help="Order ID")
    parser.add_argument("--amount", type=float, default=299.99, help="Order amount")
    parser.add_argument("--customer-id", default="customer-123", help="Customer ID")
    parser.add_argument("--timeout", type=float, default=300.0, help="Timeout in seconds")

    args = parser.parse_args()

    database_url = os.environ.get(
        "RHYTHM_DATABASE_URL", "postgresql://rhythm@localhost/rhythm"
    )

    print("Initializing Rhythm client...")
    rhythm.init(database_url=database_url)

    # Start the approval workflow
    print("\nStarting approval workflow...")
    workflow_id = rhythm.client.start_workflow(
        workflow_name="approval",
        inputs={
            "orderId": args.order_id,
            "amount": args.amount,
            "customerId": args.customer_id,
        },
    )

    print(f"\n{'=' * 60}")
    print(f"Workflow started: {workflow_id}")
    print(f"{'=' * 60}")
    print(f"\nThe workflow is now waiting for an approval signal.")
    print(f"\nTo APPROVE, run:")
    print(f"    python send_approval.py {workflow_id}")
    print(f"\nTo REJECT, run:")
    print(f"    python send_approval.py {workflow_id} --reject --reason \"Your reason\"")
    print(f"\n{'=' * 60}")

    # Wait for workflow to complete
    print(f"\nWaiting for workflow to complete (timeout: {args.timeout}s)...")
    try:
        result = rhythm.client.wait_for_execution(workflow_id, timeout=args.timeout)
        print(f"\n{'=' * 60}")
        print(f"Workflow completed!")
        print(f"Status: {result.status}")
        print(f"Output: {result.output}")
        print(f"{'=' * 60}")
    except TimeoutError:
        print(f"\nTimeout waiting for workflow. It's still running.")
        print(f"You can still send a signal with: python send_approval.py {workflow_id}")


if __name__ == "__main__":
    main()

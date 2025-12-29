#!/usr/bin/env python3
"""
Start an access request workflow.

This workflow demonstrates a human-in-the-loop access request pattern:
1. User requests access
2. Approvers are notified
3. Workflow waits for approval signal (or times out)
4. If approved, access is granted
5. Access is revoked when session ends or is canceled

Usage:
    1. Start the worker:     python worker.py
    2. Run this app:         python app.py
    3. Approve access:       python send_signal.py <workflow_id> --approve
       Or reject:            python send_signal.py <workflow_id> --reject
    4. Cancel session:       python send_signal.py <workflow_id> --cancel
"""

import argparse
import os

import rhythm


def main():
    parser = argparse.ArgumentParser(description="Start an access request workflow")
    parser.add_argument("--user-email", default="user@example.com", help="User email")
    parser.add_argument("--resource", default="production-db", help="Resource to access")
    parser.add_argument("--reason", default="Debugging issue #123", help="Access reason")
    parser.add_argument("--duration", type=int, default=60, help="Access duration in minutes")
    parser.add_argument("--timeout", type=float, default=600.0, help="Wait timeout in seconds")

    args = parser.parse_args()

    database_url = os.environ.get(
        "RHYTHM_DATABASE_URL", "postgresql://rhythm@localhost/rhythm"
    )

    print("Initializing Rhythm client...")
    rhythm.init(database_url=database_url)

    # Start the access request workflow
    print("\nStarting access request workflow...")
    workflow_id = rhythm.client.start_workflow(
        workflow_name="access_request",
        inputs={
            "user_email": args.user_email,
            "request_details": {
                "resource": args.resource,
                "reason": args.reason,
            },
            "duration_minutes": args.duration,
        },
    )

    print(f"\n{'=' * 70}")
    print(f"Access request workflow started: {workflow_id}")
    print(f"{'=' * 70}")
    print(f"\nUser: {args.user_email}")
    print(f"Resource: {args.resource}")
    print(f"Reason: {args.reason}")
    print(f"Duration: {args.duration} minutes")
    print(f"\n{'=' * 70}")
    print("WORKFLOW SIGNALS")
    print(f"{'=' * 70}")
    print(f"\nThe workflow is waiting for approval. Use send_signal.py:")
    print(f"\n  APPROVE access:")
    print(f"    python send_signal.py {workflow_id} --approve")
    print(f"\n  REJECT access:")
    print(f"    python send_signal.py {workflow_id} --reject --reason \"Reason here\"")
    print(f"\n  CANCEL session (after approval):")
    print(f"    python send_signal.py {workflow_id} --cancel")
    print(f"\n{'=' * 70}")

    # Wait for workflow to complete
    print(f"\nWaiting for workflow to complete (timeout: {args.timeout}s)...")
    try:
        result = rhythm.client.wait_for_execution(workflow_id, timeout=args.timeout)
        print(f"\n{'=' * 70}")
        print("Workflow completed!")
        print(f"Status: {result.status}")
        print(f"Output: {result.output}")
        print(f"{'=' * 70}")
    except TimeoutError:
        print(f"\nTimeout waiting for workflow. It's still running.")
        print(f"You can still send signals with: python send_signal.py {workflow_id} ...")


if __name__ == "__main__":
    main()

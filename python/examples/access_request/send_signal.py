#!/usr/bin/env python3
"""
Send signals to an access request workflow.

This script can send three types of signals:
1. Approval signal (--approve) - Approves the access request
2. Rejection signal (--reject) - Rejects the access request
3. Cancel signal (--cancel) - Cancels an active session early

Usage:
    # Approve the access request
    python send_signal.py <workflow_id> --approve

    # Reject the access request
    python send_signal.py <workflow_id> --reject --reason "Budget exceeded"

    # Cancel an active session early
    python send_signal.py <workflow_id> --cancel
"""

import argparse
import os

import rhythm


def main():
    parser = argparse.ArgumentParser(description="Send signals to an access request workflow")
    parser.add_argument("workflow_id", help="The workflow ID to send the signal to")

    action = parser.add_mutually_exclusive_group(required=True)
    action.add_argument("--approve", action="store_true", help="Approve the access request")
    action.add_argument("--reject", action="store_true", help="Reject the access request")
    action.add_argument("--cancel", action="store_true", help="Cancel an active session")

    parser.add_argument("--reason", default="", help="Reason for rejection")
    parser.add_argument("--approver", default="cli-user", help="Approver name")

    args = parser.parse_args()

    database_url = os.environ.get(
        "RHYTHM_DATABASE_URL", "postgresql://rhythm@localhost/rhythm"
    )

    rhythm.init(database_url=database_url)

    if args.approve:
        signal_name = "access-request-approval"
        payload = {
            "response": "approved",
            "approver": args.approver,
        }
        action_desc = "APPROVAL"
    elif args.reject:
        signal_name = "access-request-approval"
        payload = {
            "response": "rejected",
            "approver": args.approver,
            "reason": args.reason or "Rejected via CLI",
        }
        action_desc = "REJECTION"
    else:  # cancel
        signal_name = "access-session-canceled"
        payload = {
            "canceled_by": args.approver,
            "reason": args.reason or "Canceled via CLI",
        }
        action_desc = "CANCELLATION"

    print(f"Sending {action_desc} signal to workflow {args.workflow_id}...")
    print(f"  Signal: {signal_name}")
    print(f"  Payload: {payload}")

    rhythm.client.send_signal(
        workflow_id=args.workflow_id,
        signal_name=signal_name,
        payload=payload,
    )

    print(f"\nSignal sent successfully!")


if __name__ == "__main__":
    main()

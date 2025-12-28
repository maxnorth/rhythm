#!/usr/bin/env python3
"""
Send an approval signal to a waiting workflow.

Usage:
    python send_approval.py <workflow_id> [--reject] [--reason "reason"]

Examples:
    # Approve a workflow
    python send_approval.py abc-123

    # Reject a workflow
    python send_approval.py abc-123 --reject --reason "Budget exceeded"
"""

import argparse
import os

import rhythm


def main():
    parser = argparse.ArgumentParser(description="Send approval signal to a workflow")
    parser.add_argument("workflow_id", help="The workflow ID to send the signal to")
    parser.add_argument("--reject", action="store_true", help="Reject instead of approve")
    parser.add_argument("--reason", default="", help="Reason for rejection")
    parser.add_argument("--reviewer", default="cli-user", help="Reviewer name")

    args = parser.parse_args()

    database_url = os.environ.get(
        "RHYTHM_DATABASE_URL", "postgresql://rhythm@localhost/rhythm"
    )

    rhythm.init(database_url=database_url)

    if args.reject:
        payload = {
            "approved": False,
            "reviewer": args.reviewer,
            "reason": args.reason or "Rejected via CLI",
        }
        action = "REJECTION"
    else:
        payload = {
            "approved": True,
            "reviewer": args.reviewer,
            "notes": "Approved via CLI",
        }
        action = "APPROVAL"

    print(f"Sending {action} signal to workflow {args.workflow_id}...")
    rhythm.client.send_signal(
        workflow_id=args.workflow_id,
        signal_name="approval",
        payload=payload,
    )
    print(f"Signal sent successfully!")
    print(f"Payload: {payload}")


if __name__ == "__main__":
    main()

"""Tasks for the access_request example.

These are simple stub implementations that log instead of performing real actions.
In a production system, these would integrate with your identity provider,
notification system, and access control infrastructure.
"""

import logging
import time
import uuid

import rhythm

logger = logging.getLogger(__name__)


@rhythm.task(name="init-access-request")
def init_access_request(user_email: str, request_details: dict) -> dict:
    """Initialize an access request and return a request ID."""
    request_id = str(uuid.uuid4())[:8]
    logger.info(f"[STUB] Initializing access request {request_id}")
    logger.info(f"  User: {user_email}")
    logger.info(f"  Details: {request_details}")
    time.sleep(0.2)
    return {"request_id": request_id}


@rhythm.task(name="get-access-request-approvers")
def get_access_request_approvers(request_id: str) -> list:
    """Get the list of approvers for an access request."""
    logger.info(f"[STUB] Fetching approvers for request {request_id}")
    time.sleep(0.1)
    # Return mock approvers
    approvers = [
        {"email": "manager@example.com", "name": "Manager"},
        {"email": "security@example.com", "name": "Security Team"},
    ]
    logger.info(f"  Found {len(approvers)} approvers")
    return approvers


@rhythm.task(name="send-access-request-approval")
def send_access_request_approval(approver: dict, request_id: str) -> dict:
    """Send an approval request notification to an approver."""
    logger.info(f"[STUB] Sending approval request to {approver.get('email', approver)}")
    logger.info(f"  Request ID: {request_id}")
    time.sleep(0.1)
    return {"sent": True, "approver": approver}


@rhythm.task(name="reject-access-request")
def reject_access_request(request_id: str, reason: str) -> dict:
    """Reject an access request."""
    logger.info(f"[STUB] Rejecting access request {request_id}")
    logger.info(f"  Reason: {reason}")
    time.sleep(0.1)
    return {"rejected": True, "request_id": request_id, "reason": reason}


@rhythm.task(name="grant-access")
def grant_access(user_email: str, request_id: str) -> dict:
    """Grant access to the user."""
    logger.info(f"[STUB] Granting access to {user_email}")
    logger.info(f"  Request ID: {request_id}")
    time.sleep(0.2)
    return {"granted": True, "user_email": user_email, "request_id": request_id}


@rhythm.task(name="revoke-access")
def revoke_access(request_id: str, reason: str) -> dict:
    """Revoke access from the user."""
    logger.info(f"[STUB] Revoking access for request {request_id}")
    logger.info(f"  Reason: {reason}")
    time.sleep(0.2)
    return {"revoked": True, "request_id": request_id, "reason": reason}


@rhythm.task(name="notify-error")
def notify_error(user_email: str, message: str) -> dict:
    """Notify user of an error."""
    logger.error(f"[STUB] Notifying {user_email} of error: {message}")
    time.sleep(0.1)
    return {"notified": True, "user_email": user_email}

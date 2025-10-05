"""
Example demonstrating workflow signals for human-in-the-loop workflows
"""

import asyncio
from workflows import workflow, activity, wait_for_signal, send_signal


@activity()
async def prepare_document(doc_id: str):
    """Prepare a document for review"""
    print(f"[PREPARE] Preparing document {doc_id} for review")
    await asyncio.sleep(0.5)
    return {"prepared": True, "doc_id": doc_id}


@activity()
async def publish_document(doc_id: str):
    """Publish an approved document"""
    print(f"[PUBLISH] Publishing document {doc_id}")
    await asyncio.sleep(0.3)
    return {"published": True, "doc_id": doc_id, "url": f"https://example.com/docs/{doc_id}"}


@activity()
async def archive_document(doc_id: str):
    """Archive a rejected document"""
    print(f"[ARCHIVE] Archiving rejected document {doc_id}")
    await asyncio.sleep(0.2)
    return {"archived": True, "doc_id": doc_id}


@workflow(queue="documents", version=1, timeout=86400)  # 24 hour timeout
async def document_approval_workflow(doc_id: str, author: str):
    """
    Document approval workflow that waits for human approval.

    The workflow will suspend and wait for an external signal.
    """
    print(f"\n[WORKFLOW] Starting approval process for document {doc_id} by {author}\n")

    # Prepare the document
    prep_result = await prepare_document.run(doc_id)
    print(f"[WORKFLOW] ✓ Document prepared: {prep_result}\n")

    # Wait for approval signal (workflow suspends here)
    print(f"[WORKFLOW] ⏸  Waiting for approval signal...\n")
    approval = await wait_for_signal("approval_decision", timeout=86400)  # 24 hours
    print(f"[WORKFLOW] ▶  Received approval signal: {approval}\n")

    # Process based on approval
    if approval.get("approved"):
        publish_result = await publish_document.run(doc_id)
        print(f"[WORKFLOW] ✓ Document published: {publish_result['url']}\n")

        return {
            "status": "approved",
            "doc_id": doc_id,
            "url": publish_result["url"],
            "approved_by": approval.get("approved_by"),
        }
    else:
        archive_result = await archive_document.run(doc_id)
        print(f"[WORKFLOW] ✓ Document archived\n")

        return {
            "status": "rejected",
            "doc_id": doc_id,
            "rejected_by": approval.get("approved_by"),
            "reason": approval.get("reason"),
        }


async def main():
    """Example of starting workflow and sending signals"""
    print("=" * 60)
    print("Document Approval Workflow Example")
    print("=" * 60 + "\n")

    # Start the approval workflow
    workflow_id = await document_approval_workflow.queue(
        doc_id="doc_12345",
        author="john@example.com",
    )
    print(f"✓ Approval workflow started: {workflow_id}\n")

    print("=" * 60)
    print("Workflow is now waiting for approval signal.")
    print("Start a worker and then send a signal:")
    print(f"  workflows worker -q documents")
    print("\nTo approve:")
    print(f'  python -c "import asyncio; from workflows import send_signal; '
          f'asyncio.run(send_signal(\'{workflow_id}\', \'approval_decision\', '
          f'{{\'approved\': True, \'approved_by\': \'manager@example.com\'}}))"')
    print("\nTo reject:")
    print(f'  python -c "import asyncio; from workflows import send_signal; '
          f'asyncio.run(send_signal(\'{workflow_id}\', \'approval_decision\', '
          f'{{\'approved\': False, \'approved_by\': \'manager@example.com\', '
          f'\'reason\': \'Needs revision\'}}))"')
    print("=" * 60)


if __name__ == "__main__":
    asyncio.run(main())

#!/usr/bin/env python3
"""
Worker for the sub-workflows example.

Starts a worker that processes tasks and workflows, including
the parent and child workflows.
"""

import logging
import os

import rhythm
from rhythm.config import settings

# Configure worker settings
os.environ.setdefault("WORKFLOWS_WORKER_MAX_CONCURRENT", "1")
os.environ.setdefault("WORKFLOWS_WORKER_VERBOSE", "true")

# Import tasks to register them
import tasks  # noqa: F401

# Configure logging
logging.basicConfig(
    level=logging.DEBUG if settings.worker_verbose else logging.INFO,
    format="%(asctime)s [%(levelname)s] %(name)s: %(message)s",
)

logger = logging.getLogger(__name__)


def main():
    """Main entry point for the worker"""
    database_url = os.environ.get(
        "RHYTHM_DATABASE_URL", "postgresql://rhythm@localhost/rhythm"
    )

    logger.info("Initializing Rhythm with sub-workflows example...")
    rhythm.init(
        database_url=database_url,
        workflow_paths=["./workflows"],
        auto_migrate=True,
    )
    logger.info("Rhythm initialized - workflows registered:")
    logger.info("  - order_fulfillment (parent)")
    logger.info("  - process_payment (child)")
    logger.info("  - reserve_inventory (child)")
    logger.info("  - arrange_shipping (child)")

    logger.info("Starting worker...")
    rhythm.worker.run()


if __name__ == "__main__":
    main()

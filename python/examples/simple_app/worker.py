#!/usr/bin/env python3
"""
Worker entry point for the simple application.

Starts a worker that processes tasks and workflows.
"""

import asyncio
import logging
import os
import sys

# Configure worker settings
os.environ.setdefault("WORKFLOWS_WORKER_MAX_CONCURRENT", "1")
os.environ.setdefault("WORKFLOWS_WORKER_VERBOSE", "true")

# Add rhythm to path (for development)
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "../.."))

import rhythm
from rhythm.config import settings

# Import tasks to register them
import tasks  # noqa: F401

# Configure logging (DEBUG level if verbose mode enabled)
logging.basicConfig(
    level=logging.DEBUG if settings.worker_verbose else logging.INFO,
    format="%(asctime)s [%(levelname)s] %(name)s: %(message)s",
)

logger = logging.getLogger(__name__)


def main():
    """Main entry point for the worker"""
    # Get database URL from environment
    database_url = os.environ.get(
        "RHYTHM_DATABASE_URL", "postgresql://rhythm@localhost/rhythm"
    )

    # Initialize Rhythm (register workflows, connect to DB)
    logger.info("Initializing Rhythm...")
    rhythm.init(
        database_url=database_url,
        workflow_paths=["./workflows"],
        auto_migrate=True,
    )
    logger.info("Rhythm initialized")

    # Start the worker
    # Note: Queue configuration is set during initialization (currently "default")
    logger.info("Starting worker...")
    rhythm.worker.run()


if __name__ == "__main__":
    main()

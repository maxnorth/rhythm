"""
Initialization for Currant workflows
"""

from typing import List, Optional
from pathlib import Path

from currant.rust_bridge import RustBridge


def init(
    database_url: str,
    workflow_paths: Optional[List[str]] = None,
    auto_migrate: bool = True,
) -> None:
    """
    Initialize Currant with workflow definitions.

    This function:
    1. Initializes Rust core with database connection
    2. Scans workflow_paths for .flow files
    3. Sends workflow files to Rust core for parsing and storage

    Args:
        database_url: PostgreSQL connection string
        workflow_paths: List of paths to directories containing .flow files
        auto_migrate: Whether to automatically run migrations if needed

    Example:
        >>> import currant
        >>> currant.init(
        ...     database_url="postgresql://localhost/myapp",
        ...     workflow_paths=["./workflows", "./app/workflows"]
        ... )
    """
    workflow_paths = workflow_paths or []

    # Scan for .flow files and read their contents if paths provided
    workflows = []
    if workflow_paths:
        # Convert to Path objects
        paths = [Path(p).resolve() for p in workflow_paths]

        # Validate paths exist
        for path in paths:
            if not path.exists():
                raise ValueError(f"Workflow path does not exist: {path}")
            if not path.is_dir():
                raise ValueError(f"Workflow path is not a directory: {path}")

        # Scan for .flow files
        for path in paths:
            for flow_file in path.rglob("*.flow"):
                workflow_name = flow_file.stem  # filename without extension
                workflow_source = flow_file.read_text()
                workflows.append({
                    "name": workflow_name,
                    "source": workflow_source,
                    "file_path": str(flow_file),
                })

        if workflows:
            print(f"Found {len(workflows)} workflow(s)")
        else:
            print("No workflows found")

    # Initialize Rust core with workflows
    RustBridge.initialize(
        database_url=database_url,
        auto_migrate=auto_migrate,
        workflows=workflows if workflows else None,
    )

"""Command-line interface for workflows"""

import asyncio
import click
import logging
import sys

from workflows.rust_bridge import RustBridge
from workflows.worker import run_worker
from workflows.client import get_execution_status, list_executions, cancel_execution

# Setup logging
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(name)s: %(message)s",
    handlers=[logging.StreamHandler(sys.stdout)],
)


@click.group()
def cli():
    """Workflows - A lightweight durable execution framework"""
    pass


@cli.command()
def migrate():
    """Run database migrations"""
    click.echo("Running database migrations...")

    try:
        RustBridge.migrate()
        click.echo("✓ Migrations completed successfully")
    except Exception as e:
        click.echo(f"✗ Migration failed: {e}", err=True)
        sys.exit(1)


@cli.command()
@click.option(
    "--queue",
    "-q",
    multiple=True,
    required=True,
    help="Queue(s) to process (can specify multiple)",
)
@click.option("--worker-id", help="Worker ID (auto-generated if not provided)")
@click.option(
    "--import-module",
    "-m",
    multiple=True,
    help="Python module(s) to import (e.g. examples.simple_example)",
)
def worker(queue, worker_id, import_module):
    """Run a worker to process jobs and workflows"""
    queues = list(queue)

    # Import modules to register functions
    for module_name in import_module:
        try:
            __import__(module_name)
            click.echo(f"Imported module: {module_name}")
        except ImportError as e:
            click.echo(f"Failed to import {module_name}: {e}", err=True)
            sys.exit(1)

    click.echo(f"Starting worker for queues: {', '.join(queues)}")

    async def _run():
        try:
            await run_worker(queues, worker_id)
        except KeyboardInterrupt:
            click.echo("\nShutting down worker...")

    asyncio.run(_run())


@cli.command()
@click.argument("execution_id")
def status(execution_id):
    """Get the status of an execution"""

    async def _status():
        result = await get_execution_status(execution_id)
        if result:
            click.echo(f"Execution: {result['id']}")
            click.echo(f"Type: {result['type']}")
            click.echo(f"Function: {result['function_name']}")
            click.echo(f"Queue: {result['queue']}")
            click.echo(f"Status: {result['status']}")
            click.echo(f"Priority: {result['priority']}")
            click.echo(f"Attempts: {result['attempt']}/{result['max_retries']}")
            click.echo(f"Created: {result['created_at']}")

            if result.get("claimed_at"):
                click.echo(f"Claimed: {result['claimed_at']}")
            if result.get("completed_at"):
                click.echo(f"Completed: {result['completed_at']}")

            if result.get("result"):
                click.echo(f"\nResult:")
                click.echo(f"  {result['result']}")

            if result.get("error"):
                click.echo(f"\nError:")
                click.echo(f"  {result['error'].get('message')}")
        else:
            click.echo(f"Execution {execution_id} not found", err=True)

    asyncio.run(_status())


@cli.command(name="list")
@click.option("--queue", "-q", help="Filter by queue")
@click.option("--status", "-s", help="Filter by status")
@click.option("--limit", "-l", default=20, help="Number of results (default: 20)")
def list_cmd(queue, status, limit):
    """List executions"""

    async def _list():
        results = await list_executions(queue=queue, status=status, limit=limit)

        if not results:
            click.echo("No executions found")
            return

        click.echo(f"Found {len(results)} execution(s):\n")

        for exec in results:
            click.echo(
                f"  {exec['id'][:12]}... | {exec['type']:8} | {exec['status']:10} | "
                f"{exec['queue']:15} | {exec['function_name']}"
            )

    asyncio.run(_list())


@cli.command()
@click.argument("execution_id")
@click.confirmation_option(prompt="Are you sure you want to cancel this execution?")
def cancel(execution_id):
    """Cancel a pending or suspended execution"""

    async def _cancel():
        success = await cancel_execution(execution_id)
        if success:
            click.echo(f"✓ Execution {execution_id} cancelled")
        else:
            click.echo(
                f"✗ Could not cancel execution {execution_id} (not found or not cancellable)",
                err=True,
            )

    asyncio.run(_cancel())


if __name__ == "__main__":
    cli()

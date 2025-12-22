"""
Rhythm CLI entry point

This module provides a thin wrapper around the Rust CLI implementation.
All CLI logic is implemented in Rust core for consistency across language adapters.
"""

import asyncio
import os
import sys

import click


@click.group()
@click.option("--database-url", envvar="RHYTHM_DATABASE_URL", help="Database URL")
@click.option("--config", envvar="RHYTHM_CONFIG_PATH", help="Config file path")
def main(database_url, config):
    """Rhythm workflow engine"""
    if database_url:
        os.environ["RHYTHM_DATABASE_URL"] = database_url
    if config:
        os.environ["RHYTHM_CONFIG_PATH"] = config


@main.command()
@click.option("-q", "--queue", "queues", multiple=True, required=True, help="Queue to process")
@click.option("--worker-id", help="Unique worker ID")
@click.option("-m", "--import", "import_modules", multiple=True, help="Module to import")
def worker(queues, worker_id, import_modules):
    """Run a worker to process tasks"""
    # Import modules to register decorated functions
    for module_name in import_modules:
        try:
            __import__(module_name)
            click.echo(f"Imported module: {module_name}")
        except ImportError as e:
            click.echo(f"Failed to import {module_name}: {e}", err=True)
            sys.exit(1)

    click.echo(f"Starting worker for queues: {', '.join(queues)}")

    from rhythm.worker import run_worker

    async def _run():
        try:
            await run_worker()
        except KeyboardInterrupt:
            click.echo("\nShutting down worker...")

    try:
        asyncio.run(_run())
    except KeyboardInterrupt:
        click.echo("\nInterrupted")
        sys.exit(130)


if __name__ == "__main__":
    main()

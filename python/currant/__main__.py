"""
Currant CLI entry point

This module provides a thin wrapper around the Rust CLI implementation.
All CLI logic is implemented in Rust core for consistency across language adapters.
"""

import sys


def main():
    """
    Entry point for the Currant CLI.

    This is intentionally minimal - all CLI parsing and logic happens in Rust.
    We normalize sys.argv and pass it to Rust for parsing.
    """
    # Normalize argv for Rust CLI
    # When run as 'python -m currant migrate', Python sets:
    # sys.argv = ['/path/to/currant/__main__.py', 'migrate', ...]
    # We want: args = ['currant', 'migrate', ...]
    args = sys.argv.copy()
    args[0] = 'currant'

    # Special handling for worker command - needs Python-specific logic
    if len(args) > 1 and args[1] == "worker":
        import asyncio
        from currant.worker import run_worker

        # Parse worker args
        queues = []
        worker_id = None
        import_modules = []

        i = 2
        while i < len(args):
            if args[i] in ("-q", "--queue"):
                if i + 1 < len(args):
                    queues.append(args[i + 1])
                    i += 2
                else:
                    print("Error: --queue requires a value", file=sys.stderr)
                    sys.exit(1)
            elif args[i] == "--worker-id":
                if i + 1 < len(args):
                    worker_id = args[i + 1]
                    i += 2
                else:
                    print("Error: --worker-id requires a value", file=sys.stderr)
                    sys.exit(1)
            elif args[i] in ("-m", "--import"):
                if i + 1 < len(args):
                    import_modules.append(args[i + 1])
                    i += 2
                else:
                    print("Error: --import requires a value", file=sys.stderr)
                    sys.exit(1)
            else:
                print(f"Error: Unknown argument: {args[i]}", file=sys.stderr)
                sys.exit(1)

        if not queues:
            print("Error: At least one queue is required (-q/--queue)", file=sys.stderr)
            sys.exit(1)

        # Import modules to register decorated functions
        for module_name in import_modules:
            try:
                __import__(module_name)
                print(f"Imported module: {module_name}")
            except ImportError as e:
                print(f"Failed to import {module_name}: {e}", file=sys.stderr)
                sys.exit(1)

        print(f"Starting worker for queues: {', '.join(queues)}")

        # Run the worker
        async def _run():
            try:
                await run_worker(queues, worker_id)
            except KeyboardInterrupt:
                print("\nShutting down worker...")

        try:
            asyncio.run(_run())
        except KeyboardInterrupt:
            print("\nInterrupted")
            sys.exit(130)
    else:
        # All other commands handled by Rust CLI
        from currant.rust_bridge import RustBridge

        try:
            RustBridge.run_cli(args)
        except KeyboardInterrupt:
            print("\nInterrupted")
            sys.exit(130)
        except Exception as e:
            print(f"Error: {e}", file=sys.stderr)
            sys.exit(1)


if __name__ == "__main__":
    main()

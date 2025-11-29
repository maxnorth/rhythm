#!/usr/bin/env python3
"""
Build and publish script for the Rhythm Python package.

This script handles:
- Building the package with maturin
- Optionally bumping the version
- Publishing to PyPI or TestPyPI
- Creating git tags

Usage:
    # Build only (no publish)
    python scripts/publish.py --build-only

    # Build and publish to TestPyPI
    python scripts/publish.py --test

    # Build and publish to PyPI (production)
    python scripts/publish.py --publish

    # Bump version and publish
    python scripts/publish.py --bump-version patch --publish

    # Specify version explicitly
    python scripts/publish.py --version 0.2.0 --publish
"""

import argparse
import re
import subprocess
import sys
from pathlib import Path
from typing import Optional


class Colors:
    """ANSI color codes for terminal output."""
    HEADER = '\033[95m'
    OKBLUE = '\033[94m'
    OKCYAN = '\033[96m'
    OKGREEN = '\033[92m'
    WARNING = '\033[93m'
    FAIL = '\033[91m'
    ENDC = '\033[0m'
    BOLD = '\033[1m'


def print_step(message: str):
    """Print a step message in blue."""
    print(f"\n{Colors.OKBLUE}{Colors.BOLD}==> {message}{Colors.ENDC}")


def print_success(message: str):
    """Print a success message in green."""
    print(f"{Colors.OKGREEN}✓ {message}{Colors.ENDC}")


def print_error(message: str):
    """Print an error message in red."""
    print(f"{Colors.FAIL}✗ {message}{Colors.ENDC}", file=sys.stderr)


def print_warning(message: str):
    """Print a warning message in yellow."""
    print(f"{Colors.WARNING}⚠ {message}{Colors.ENDC}")


def run_command(cmd: list[str], cwd: Optional[Path] = None, check: bool = True, interactive: bool = False) -> subprocess.CompletedProcess:
    """Run a shell command and return the result.

    Args:
        cmd: Command and arguments as a list
        cwd: Working directory for the command
        check: Whether to exit on non-zero return code
        interactive: If True, don't capture output (for interactive commands)
    """
    print(f"  Running: {' '.join(cmd)}")

    if interactive:
        # Don't capture output for interactive commands
        result = subprocess.run(cmd, cwd=cwd)
    else:
        result = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True)

    if check and result.returncode != 0:
        print_error(f"Command failed with exit code {result.returncode}")
        if not interactive and result.stdout:
            print(f"stdout: {result.stdout}")
        if not interactive and result.stderr:
            print(f"stderr: {result.stderr}")
        sys.exit(1)

    return result


def get_project_root() -> Path:
    """Get the project root directory."""
    script_dir = Path(__file__).parent
    return script_dir.parent


def get_current_version() -> str:
    """Get the current version from pyproject.toml."""
    pyproject_path = get_project_root() / "pyproject.toml"
    content = pyproject_path.read_text()

    match = re.search(r'version\s*=\s*"([^"]+)"', content)
    if not match:
        print_error("Could not find version in pyproject.toml")
        sys.exit(1)

    return match.group(1)


def bump_version(current: str, bump_type: str) -> str:
    """Bump the version number.

    Args:
        current: Current version string (e.g., "0.1.0")
        bump_type: One of "major", "minor", "patch"

    Returns:
        New version string
    """
    parts = current.split('.')
    if len(parts) != 3:
        print_error(f"Invalid version format: {current}")
        sys.exit(1)

    major, minor, patch = map(int, parts)

    if bump_type == "major":
        major += 1
        minor = 0
        patch = 0
    elif bump_type == "minor":
        minor += 1
        patch = 0
    elif bump_type == "patch":
        patch += 1
    else:
        print_error(f"Invalid bump type: {bump_type}")
        sys.exit(1)

    return f"{major}.{minor}.{patch}"


def set_version(version: str):
    """Update the version in pyproject.toml."""
    print_step(f"Updating version to {version}")

    pyproject_path = get_project_root() / "pyproject.toml"
    content = pyproject_path.read_text()

    new_content = re.sub(
        r'(version\s*=\s*)"[^"]+"',
        rf'\1"{version}"',
        content
    )

    pyproject_path.write_text(new_content)
    print_success(f"Version updated to {version}")


def clean_build_artifacts():
    """Clean previous build artifacts."""
    print_step("Cleaning build artifacts")

    project_root = get_project_root()

    # Clean dist directory
    dist_dir = project_root / "dist"
    if dist_dir.exists():
        run_command(["rm", "-rf", str(dist_dir)])

    # Clean target directory
    target_dir = project_root / "target"
    if target_dir.exists():
        run_command(["rm", "-rf", str(target_dir)])

    print_success("Build artifacts cleaned")


def build_package(release: bool = True):
    """Build the package using maturin."""
    print_step("Building package with maturin")

    project_root = get_project_root()

    cmd = ["maturin", "build"]
    if release:
        cmd.append("--release")

    run_command(cmd, cwd=project_root, interactive=True)
    print_success("Package built successfully")


def publish_package(test: bool = False):
    """Publish the package to PyPI or TestPyPI."""
    repository = "testpypi" if test else "pypi"
    print_step(f"Publishing to {repository}")

    project_root = get_project_root()

    cmd = ["maturin", "publish"]
    if test:
        cmd.extend(["--repository", "testpypi"])

    # Note: This will prompt for credentials or use token from environment
    # Use interactive=True to allow prompts to be shown
    run_command(cmd, cwd=project_root, interactive=True)
    print_success(f"Package published to {repository}")


def create_git_tag(version: str):
    """Create and push a git tag for the version."""
    print_step(f"Creating git tag v{version}")

    project_root = get_project_root()

    # Check if there are uncommitted changes
    result = run_command(["git", "status", "--porcelain"], cwd=project_root, check=False)
    if result.stdout.strip():
        print_warning("There are uncommitted changes. Commit them before tagging.")
        response = input("Continue anyway? (y/N): ")
        if response.lower() != 'y':
            print("Aborted.")
            sys.exit(0)

    # Create tag
    run_command(["git", "tag", "-a", f"v{version}", "-m", f"Release v{version}"], cwd=project_root)

    # Ask to push tag
    response = input(f"Push tag v{version} to remote? (y/N): ")
    if response.lower() == 'y':
        run_command(["git", "push", "origin", f"v{version}"], cwd=project_root)
        print_success(f"Tag v{version} pushed to remote")
    else:
        print_warning(f"Tag v{version} created locally but not pushed")


def main():
    parser = argparse.ArgumentParser(
        description="Build and publish the Rhythm Python package",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__
    )

    parser.add_argument(
        "--build-only",
        action="store_true",
        help="Only build the package, don't publish"
    )

    parser.add_argument(
        "--test",
        action="store_true",
        help="Publish to TestPyPI instead of PyPI"
    )

    parser.add_argument(
        "--publish",
        action="store_true",
        help="Publish to PyPI (production)"
    )

    parser.add_argument(
        "--version",
        type=str,
        help="Set a specific version (e.g., 0.2.0)"
    )

    parser.add_argument(
        "--bump-version",
        choices=["major", "minor", "patch"],
        help="Bump the version before building"
    )

    parser.add_argument(
        "--no-clean",
        action="store_true",
        help="Don't clean build artifacts before building"
    )

    parser.add_argument(
        "--tag",
        action="store_true",
        help="Create a git tag for the version"
    )

    parser.add_argument(
        "--dev",
        action="store_true",
        help="Build in development mode (not release)"
    )

    args = parser.parse_args()

    # Validate arguments
    if args.publish and args.test:
        print_error("Cannot use both --publish and --test")
        sys.exit(1)

    if args.version and args.bump_version:
        print_error("Cannot use both --version and --bump-version")
        sys.exit(1)

    # Get current version
    current_version = get_current_version()
    print(f"Current version: {current_version}")

    # Update version if requested
    new_version = current_version
    if args.version:
        new_version = args.version
        set_version(new_version)
    elif args.bump_version:
        new_version = bump_version(current_version, args.bump_version)
        set_version(new_version)

    # Clean build artifacts
    if not args.no_clean:
        clean_build_artifacts()

    # Build package
    build_package(release=not args.dev)

    # Publish if requested
    if args.publish or args.test:
        if args.test:
            print_warning("Publishing to TestPyPI")
        publish_package(test=args.test)

    # Create git tag if requested
    if args.tag:
        create_git_tag(new_version)

    print()
    print_success("All done!")
    print(f"Version: {new_version}")

    if args.build_only:
        print(f"Package built in: {get_project_root() / 'dist'}")
    elif args.test:
        print("Published to: https://test.pypi.org/project/rhythm/")
    elif args.publish:
        print("Published to: https://pypi.org/project/rhythm/")


if __name__ == "__main__":
    main()

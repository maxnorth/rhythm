#!/usr/bin/env python3
"""Render API reference JSON to Markdown documentation.

This script validates API reference JSON against the schema and renders
it to formatted Markdown documentation.
"""

import argparse
import json
import sys
from pathlib import Path
from typing import Any, Dict, List

try:
    import jsonschema
except ImportError:
    print("Error: jsonschema package is required. Install with: pip install jsonschema", file=sys.stderr)
    sys.exit(1)

try:
    import yaml
except ImportError:
    print("Error: pyyaml package is required. Install with: pip install pyyaml", file=sys.stderr)
    sys.exit(1)


def load_schema(schema_path: Path) -> Dict[str, Any]:
    """Load schema from file (supports JSON and YAML)."""
    with open(schema_path) as f:
        if schema_path.suffix in ['.yml', '.yaml']:
            return yaml.safe_load(f)
        else:
            return json.load(f)


def load_api_json(json_path: Path) -> Dict[str, Any]:
    """Load API reference JSON from file."""
    with open(json_path) as f:
        return json.load(f)


def validate_json(data: Dict[str, Any], schema: Dict[str, Any]) -> None:
    """Validate JSON data against schema.

    Raises:
        jsonschema.ValidationError: If validation fails
    """
    jsonschema.validate(instance=data, schema=schema)


def render_parameter(param: Dict[str, str]) -> str:
    """Render a single parameter to markdown."""
    return f"- **`{param['name']}`**: {param['description']}"


def render_parameters(parameters: List[Dict[str, str]]) -> str:
    """Render parameters list to markdown."""
    if not parameters:
        return ""

    lines = ["**Parameters:**\n"]
    lines.extend(render_parameter(p) for p in parameters)
    return "\n".join(lines)


def render_returns(returns: str) -> str:
    """Render returns section to markdown."""
    if not returns:
        return ""
    return f"**Returns:** {returns}"


def render_raises(raises: List[str]) -> str:
    """Render raises section to markdown."""
    if not raises:
        return ""

    lines = ["**Raises:**\n"]
    lines.extend(f"- `{exc}`" for exc in raises)
    return "\n".join(lines)


def render_usage(usage: str) -> str:
    """Render usage example to markdown."""
    if not usage:
        return ""

    return f"**Example:**\n\n```python\n{usage}\n```"


def render_item(item: Dict[str, Any]) -> str:
    """Render a single API item to markdown."""
    lines = []

    # Header with name and kind badge
    kind_badge = f"`{item['kind']}`"
    lines.append(f"### {item['name']} {kind_badge}\n")

    # Signature
    lines.append(f"```python\n{item['name']}{item['signature']}\n```\n")

    # Description
    lines.append(f"{item['description']}\n")

    # Parameters
    if item.get('parameters'):
        lines.append(render_parameters(item['parameters']))
        lines.append("")

    # Returns
    if item.get('returns'):
        lines.append(render_returns(item['returns']))
        lines.append("")

    # Raises
    if item.get('raises'):
        lines.append(render_raises(item['raises']))
        lines.append("")

    # Usage example
    if item.get('usage'):
        lines.append(render_usage(item['usage']))
        lines.append("")

    return "\n".join(lines)


def render_section(section: Dict[str, Any]) -> str:
    """Render a section with all its items to markdown."""
    lines = []

    # Section header
    lines.append(f"## {section['title']}\n")

    # Render each item
    for item in section['items']:
        lines.append(render_item(item))
        lines.append("---\n")

    return "\n".join(lines)


def render_to_markdown(data: Dict[str, Any]) -> str:
    """Render the entire API reference to markdown."""
    lines = []

    # Document title and summary
    lines.append(f"# {data['title']}\n")
    lines.append(f"{data['summary']}\n")
    lines.append("---\n")

    # Render each section
    for section in data['sections']:
        lines.append(render_section(section))

    return "\n".join(lines)


def main():
    """Main entry point."""
    parser = argparse.ArgumentParser(
        description="Render API reference JSON to Markdown documentation"
    )
    parser.add_argument(
        "json_file",
        type=Path,
        help="Path to API reference JSON file"
    )
    parser.add_argument(
        "output_file",
        type=Path,
        help="Path to output Markdown file"
    )
    parser.add_argument(
        "--schema",
        type=Path,
        default=Path(__file__).parent / "reference.schema.yml",
        help="Path to schema file (default: reference.schema.yml in script directory)"
    )
    parser.add_argument(
        "--no-validate",
        action="store_true",
        help="Skip JSON schema validation"
    )

    args = parser.parse_args()

    # Check input file exists
    if not args.json_file.exists():
        print(f"Error: Input file not found: {args.json_file}", file=sys.stderr)
        sys.exit(1)

    # Load API JSON
    print(f"Loading API reference from {args.json_file}...")
    try:
        api_data = load_api_json(args.json_file)
    except json.JSONDecodeError as e:
        print(f"Error: Invalid JSON in {args.json_file}: {e}", file=sys.stderr)
        sys.exit(1)

    # Validate against schema
    if not args.no_validate:
        if not args.schema.exists():
            print(f"Warning: Schema file not found: {args.schema}", file=sys.stderr)
            print("Skipping validation...", file=sys.stderr)
        else:
            print(f"Validating against schema {args.schema}...")
            try:
                schema = load_schema(args.schema)
                validate_json(api_data, schema)
                print("✓ Validation successful")
            except jsonschema.ValidationError as e:
                print(f"Error: Validation failed: {e.message}", file=sys.stderr)
                print(f"Path: {' -> '.join(str(p) for p in e.path)}", file=sys.stderr)
                sys.exit(1)

    # Render to markdown
    print("Rendering to Markdown...")
    markdown = render_to_markdown(api_data)

    # Write output
    args.output_file.parent.mkdir(parents=True, exist_ok=True)
    with open(args.output_file, 'w') as f:
        f.write(markdown)

    print(f"✓ Generated: {args.output_file}")
    print(f"  Sections: {len(api_data['sections'])}")
    total_items = sum(len(s['items']) for s in api_data['sections'])
    print(f"  Total items: {total_items}")


if __name__ == "__main__":
    main()

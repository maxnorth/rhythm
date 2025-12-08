#!/usr/bin/env python3
"""Render API reference YAML to Markdown documentation.

This script validates API reference YAML against the schema and renders
it to formatted Markdown documentation.
"""

import argparse
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


def load_api_data(data_path: Path) -> Dict[str, Any]:
    """Load API reference data from file (supports JSON and YAML)."""
    with open(data_path) as f:
        if data_path.suffix in ['.yml', '.yaml']:
            return yaml.safe_load(f)
        else:
            # Fallback to JSON for backwards compatibility
            import json
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


def render_item(item: Dict[str, Any], section_title: str = "") -> str:
    """Render a single API item to markdown."""
    lines = []

    # Header with section prefix and kind badge
    kind_badge = f"`{item['kind']}`"
    # Use HTML anchor for precise control over ID
    if section_title:
        from html import escape
        anchor_id = generate_anchor(f"{section_title} {item['name']}")
        lines.append(f"### <a id=\"{anchor_id}\"></a>{item['name']} {kind_badge}\n")
    else:
        lines.append(f"### {item['name']} {kind_badge}\n")

    # Signature (only include if present)
    if item.get('signature'):
        sig = item['signature']
        # Check if signature is a complete expression (contains dot, colon, or starts with paren)
        # This handles workflow API items like "Task.run(...)" or "ctx: object"
        if '.' in sig or sig.startswith('(') or ': ' in sig:
            # Already a complete signature, don't prepend name
            lines.append(f"```\n{sig}\n```\n")
        else:
            # Python API style - prepend name to signature
            lines.append(f"```python\n{item['name']}{sig}\n```\n")

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

    # Usage example (single string)
    if item.get('usage'):
        lines.append(render_usage(item['usage']))
        lines.append("")

    # Examples (array of Example objects)
    if item.get('examples'):
        if len(item['examples']) == 1:
            lines.append("**Example:**\n")
        else:
            lines.append("**Examples:**\n")
        for example in item['examples']:
            # Reuse section example renderer
            example_md = render_section_example(example)
            # Indent the example content slightly
            lines.append(example_md)

    return "\n".join(lines)


def render_section_example(example: Dict[str, Any]) -> str:
    """Render a single section example to markdown."""
    lines = []

    if example.get('title'):
        lines.append(f"**{example['title']}**")

    if example.get('description'):
        lines.append(f"{example['description']}\n")

    lines.append(f"```python\n{example['code']}\n```\n")

    return "\n".join(lines)


def render_section(section: Dict[str, Any]) -> str:
    """Render a section with all its items to markdown."""
    lines = []

    # Section header
    lines.append(f"## {section['title']}\n")

    # Section description if present
    if section.get('description'):
        lines.append(f"{section['description']}\n")

    # Section examples if present
    if section.get('examples'):
        for example in section['examples']:
            lines.append(render_section_example(example))

    # Render each item with section title for anchor generation
    items = section['items']
    for i, item in enumerate(items):
        lines.append(render_item(item, section['title']))
        # Add subtle divider between items (but not after the last one)
        if i < len(items) - 1:
            lines.append("* * *\n")

    return "\n".join(lines)


def generate_anchor(text: str) -> str:
    """Generate markdown anchor link from text."""
    # GitHub-flavored markdown anchor: lowercase, spaces to dots, remove special chars
    anchor = text.lower().replace(' ', '.')
    # Remove any non-alphanumeric characters except dots, hyphens and underscores
    anchor = ''.join(c for c in anchor if c.isalnum() or c in '.-_')
    return anchor


def render_table_of_contents(data: Dict[str, Any]) -> str:
    """Generate table of contents with links to sections and items."""
    lines = ["### Table of Contents\n"]

    for section in data['sections']:
        section_anchor = generate_anchor(section['title'])
        lines.append(f"- [{section['title']}](#{section_anchor})")

        # Add items within this section
        for item in section['items']:
            # Prefix item with section name to avoid conflicts
            item_header_text = f"{section['title']} {item['name']}"
            item_anchor = generate_anchor(item_header_text)
            lines.append(f"  - [{item['name']}](#{item_anchor})")

    lines.append("")  # Empty line after TOC
    return "\n".join(lines)


def render_to_markdown(data: Dict[str, Any]) -> str:
    """Render the entire API reference to markdown."""
    lines = []

    # Document title and summary
    lines.append(f"# {data['title']}\n")
    lines.append(f"{data['summary']}\n")

    # Table of contents
    lines.append(render_table_of_contents(data))

    # Render each section
    for section in data['sections']:
        lines.append(render_section(section))

    return "\n".join(lines)


def main():
    """Main entry point."""
    parser = argparse.ArgumentParser(
        description="Render API reference YAML to Markdown documentation"
    )
    parser.add_argument(
        "input_file",
        type=Path,
        help="Path to API reference YAML file"
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
        help="Skip schema validation"
    )

    args = parser.parse_args()

    # Check input file exists
    if not args.input_file.exists():
        print(f"Error: Input file not found: {args.input_file}", file=sys.stderr)
        sys.exit(1)

    # Load API data
    print(f"Loading API reference from {args.input_file}...")
    try:
        api_data = load_api_data(args.input_file)
    except Exception as e:
        print(f"Error: Failed to load {args.input_file}: {e}", file=sys.stderr)
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

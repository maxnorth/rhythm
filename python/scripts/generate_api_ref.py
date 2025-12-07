#!/usr/bin/env python3
"""Generate API reference JSON from Python modules.

Extracts API metadata from Python source code to JSON format.
"""

import inspect
import json
import re
from typing import Any, Dict, List, Optional
from pathlib import Path
import sys

try:
    import yaml
except ImportError:
    print("Error: pyyaml package is required. Install with: pip install pyyaml", file=sys.stderr)
    sys.exit(1)

# Add parent directory to path to import rhythm
sys.path.insert(0, str(Path(__file__).parent.parent))

import rhythm
import rhythm.client as client_module
import rhythm.worker as worker_module
import rhythm.decorators as decorators_module
from importlib import import_module
init_module = import_module('rhythm.init')

# These will be populated from supplemental docs YAML
VALID_SECTIONS = set()
SECTION_ORDER = []
SUPPLEMENTAL_DOCS = {}


def load_supplemental_docs(yaml_path: Path) -> Dict[str, Any]:
    """Load supplemental documentation content from YAML file."""
    with open(yaml_path) as f:
        return yaml.safe_load(f)


def initialize_from_supplemental_docs(docs: Dict[str, Any]) -> None:
    """Initialize global section data from supplemental docs."""
    global VALID_SECTIONS, SECTION_ORDER, SUPPLEMENTAL_DOCS

    SUPPLEMENTAL_DOCS = docs

    # Extract section names and order
    for section in docs.get('sections', []):
        section_name = section['name']
        SECTION_ORDER.append(section_name)
        VALID_SECTIONS.add(section_name)


def parse_google_section(docstring: str, section_name: str) -> Optional[str]:
    """Extract content from a Google-style docstring section.

    Args:
        docstring: The full docstring
        section_name: Name of section to extract (e.g., 'Args', 'Returns', 'Meta')

    Returns:
        Section content if found, None otherwise
    """
    # Match section header followed by content until next section or end
    pattern = rf'^{section_name}:\s*\n(.*?)(?=\n\w+:\s*\n|\Z)'
    match = re.search(pattern, docstring, re.MULTILINE | re.DOTALL)

    if match:
        return match.group(1).strip()
    return None


def parse_args_section(content: str) -> List[Dict[str, str]]:
    """Parse Google-style Args section into parameter list."""
    parameters = []
    if not content:
        return parameters

    # Match parameter lines: "name: description" (with optional leading whitespace)
    param_pattern = r'^\s*(\w+):\s*(.+?)(?=\n\s*\w+:|\Z)'

    for match in re.finditer(param_pattern, content, re.MULTILINE | re.DOTALL):
        param_name = match.group(1)
        param_desc = match.group(2).strip()
        # Remove internal line breaks and extra whitespace
        param_desc = re.sub(r'\s+', ' ', param_desc)

        parameters.append({
            "name": param_name,
            "description": param_desc
        })

    return parameters


def parse_raises_section(content: str) -> List[str]:
    """Parse Google-style Raises section."""
    raises = []
    if not content:
        return raises

    # Match "ExceptionType: description" lines (with optional leading whitespace)
    raise_pattern = r'^\s*(\w+):\s*(.+?)(?=\n\s*\w+:|\Z)'

    for match in re.finditer(raise_pattern, content, re.MULTILINE | re.DOTALL):
        exc_type = match.group(1)
        exc_desc = match.group(2).strip()
        exc_desc = re.sub(r'\s+', ' ', exc_desc)
        raises.append(f"{exc_type}: {exc_desc}")

    return raises


def parse_meta_section(content: str) -> Dict[str, str]:
    """Parse Meta section for documentation metadata."""
    metadata = {}
    if not content:
        return metadata

    # Match "key: value" lines (with optional leading whitespace)
    meta_pattern = r'^\s*(\w+):\s*(.+?)$'

    for match in re.finditer(meta_pattern, content, re.MULTILINE):
        key = match.group(1)
        value = match.group(2).strip()
        metadata[key] = value

    return metadata


def parse_docstring(docstring: str) -> Dict[str, Any]:
    """Parse a Google-style docstring into structured components."""
    if not docstring:
        return {
            "description": "",
            "metadata": {},
            "parameters": [],
            "returns": None,
            "raises": [],
            "example": None
        }

    # Extract description (everything before first section)
    desc_match = re.match(r'^(.*?)(?=\n\w+:\s*\n|\Z)', docstring, re.DOTALL)
    description = desc_match.group(1).strip() if desc_match else ""

    # Parse sections
    args_content = parse_google_section(docstring, 'Args')
    returns_content = parse_google_section(docstring, 'Returns')
    raises_content = parse_google_section(docstring, 'Raises')
    example_content = parse_google_section(docstring, 'Example')
    meta_content = parse_google_section(docstring, 'Meta')

    # Parse structured content
    parameters = parse_args_section(args_content) if args_content else []
    raises = parse_raises_section(raises_content) if raises_content else []
    metadata = parse_meta_section(meta_content) if meta_content else {}

    return {
        "description": description,
        "metadata": metadata,
        "parameters": parameters,
        "returns": returns_content,
        "raises": raises,
        "example": example_content
    }


def extract_function(func: Any) -> Dict[str, Any]:
    """Extract metadata from a function."""
    sig = inspect.signature(func)
    docstring = inspect.getdoc(func)
    parsed = parse_docstring(docstring or "")

    # Get doc metadata from Meta section
    metadata = parsed['metadata']

    # Get section assignment and validate
    section = metadata.get('section')

    if not section:
        raise ValueError(
            f"Function '{func.__name__}' in {func.__module__} is missing 'section' in Meta. "
            f"Valid sections: {', '.join(sorted(VALID_SECTIONS))}"
        )

    if section not in VALID_SECTIONS:
        raise ValueError(
            f"Function '{func.__name__}' in {func.__module__} has invalid section '{section}'. "
            f"Valid sections: {', '.join(sorted(VALID_SECTIONS))}"
        )

    # Build item structure matching schema
    item = {
        "kind": metadata.get('kind', 'function'),
        "name": func.__name__,
        "signature": str(sig),
        "description": parsed['description'],
        "parameters": parsed['parameters']
    }

    # Add optional fields
    if parsed['returns']:
        item['returns'] = parsed['returns']

    if parsed['raises']:
        item['raises'] = parsed['raises']

    # Add usage example if specified
    if parsed['example']:
        item['usage'] = parsed['example']

    # Add section assignment
    item['_section'] = section

    return item


def extract_module(module: Any, name: str, public_only: bool = True) -> List[Dict[str, Any]]:
    """Extract all items from a module."""
    items = []

    # Get all functions from module
    for item_name, item in inspect.getmembers(module, inspect.isfunction):
        # Skip private functions if public_only
        if public_only and item_name.startswith('_'):
            continue

        # Only include functions defined in this module
        if item.__module__ == module.__name__:
            items.append(extract_function(item))

    return items


def organize_into_sections(items: List[Dict[str, Any]]) -> List[Dict[str, Any]]:
    """Organize items into sections and merge with supplemental docs."""
    # Group by section
    sections_dict = {}

    for item in items:
        section_name = item.pop('_section', 'Other')

        if section_name not in sections_dict:
            sections_dict[section_name] = {
                "title": section_name,
                "items": []
            }

        sections_dict[section_name]["items"].append(item)

    # Build sections in predefined order, merging supplemental content
    sections = []
    for section_config in SUPPLEMENTAL_DOCS.get('sections', []):
        section_name = section_config['name']

        if section_name in sections_dict:
            # Sort items within section alphabetically by name
            sorted_items = sorted(sections_dict[section_name]["items"], key=lambda x: x['name'])

            section_data = {
                "title": section_name,
                "items": sorted_items
            }

            # Add supplemental content if present
            if 'description' in section_config:
                section_data['description'] = section_config['description'].strip()

            if 'examples' in section_config:
                section_data['examples'] = section_config['examples']

            sections.append(section_data)

    return sections


def main():
    """Extract API metadata and generate documentation."""

    # Load supplemental documentation
    docs_yaml_path = Path(__file__).parent.parent / "docs" / "api-docs.yml"
    if not docs_yaml_path.exists():
        print(f"Error: Supplemental docs YAML not found: {docs_yaml_path}", file=sys.stderr)
        sys.exit(1)

    print(f"Loading supplemental docs from {docs_yaml_path}...")
    supplemental_docs = load_supplemental_docs(docs_yaml_path)
    initialize_from_supplemental_docs(supplemental_docs)

    modules_to_extract = [
        (rhythm, "rhythm"),
        (init_module, "rhythm.init"),
        (decorators_module, "rhythm.decorators"),
        (client_module, "rhythm.client"),
        (worker_module, "rhythm.worker"),
    ]

    all_items = []

    print("Extracting API metadata...")
    for module, name in modules_to_extract:
        print(f"  - {name}")
        items = extract_module(module, name)
        all_items.extend(items)

    # Organize into sections
    sections = organize_into_sections(all_items)

    # Build final document structure
    api_doc = {
        "title": "Python API Reference",
        "summary": "Complete API reference for the Rhythm Python SDK",
        "sections": sections
    }

    # Create output directory
    output_dir = Path(__file__).parent.parent / "docs"
    output_dir.mkdir(exist_ok=True)

    # Save JSON
    json_path = output_dir / "python-api.json"
    with open(json_path, 'w') as f:
        json.dump(api_doc, f, indent=2)

    print(f"\n✓ Generated: {json_path}")
    print(f"\nExtracted {len(sections)} sections with {len(all_items)} total items:")
    for section in sections:
        print(f"  - {section['title']}: {len(section['items'])} items")


if __name__ == "__main__":
    main()

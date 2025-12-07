# API Documentation Generation

This directory contains tools for generating and rendering API documentation.

## Overview

The documentation system combines:
- **Code extraction**: Automatically extracts API metadata from Python docstrings ([`python/scripts/generate_api_ref.py`](../../python/scripts/generate_api_ref.py))
- **Supplemental content**: YAML file for section descriptions, examples, and organization ([`python/docs/api-docs.yml`](../../python/docs/api-docs.yml))
- **JSON schema validation**: Ensures documentation structure is correct
- **Markdown rendering**: Produces formatted documentation with table of contents

## Files

- **`reference.schema.yml`**: JSON Schema (YAML format) defining the structure of API reference JSON files
- **`render_api_docs.py`**: Script to validate and render API JSON to Markdown

## Related Files

- **`python/docs/api-docs.yml`**: Supplemental documentation content (section descriptions, examples, ordering)
- **`python/scripts/generate_api_ref.py`**: Extracts API metadata from Python source code

## Usage

### Complete Pipeline

Generate the complete documentation from source code:

```bash
# From project root
make python-docs
```

This runs both extraction and rendering:
1. Extracts API metadata from Python docstrings
2. Merges with supplemental content from `python/docs/api-docs.yml`
3. Validates against schema
4. Renders to Markdown with table of contents

### Supplemental Documentation Content

The `python/docs/api-docs.yml` file defines:
- **Section order**: The order sections appear in documentation
- **Section descriptions**: Overview text for each section
- **Section examples**: Code examples that appear at the section level

**Example:**

```yaml
sections:
  - name: Initialization
    description: |
      Initialize Rhythm and configure your application.
    examples:
      - title: Basic initialization
        description: Initialize with database connection
        code: |
          import rhythm
          rhythm.init("postgresql://user@localhost/db")
```

### Rendering API Documentation

Convert an API reference JSON file to Markdown:

```bash
python docs/gen/render_api_docs.py <input.json> <output.md>
```

**Example:**

```bash
# From project root
python docs/gen/render_api_docs.py python/docs/python-api.json docs/python-api.md
```

**Options:**

- `--schema <path>`: Path to schema file (default: `reference.schema.yml` in script directory)
- `--no-validate`: Skip JSON schema validation

### Validating API JSON

The script automatically validates the input JSON against the schema. To only validate without rendering, you can use `jsonschema` directly:

```bash
# From project root
jsonschema -i python/docs/python-api.json docs/gen/reference.schema.yml
```

## Requirements

- Python 3.8+
- `jsonschema` package: `pip install jsonschema`
- `pyyaml` package: `pip install pyyaml`

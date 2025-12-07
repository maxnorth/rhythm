# API Documentation Generation

This directory contains tools for generating and rendering API documentation.

## Files

- **`reference.schema.yml`**: JSON Schema (YAML format) defining the structure of API reference JSON files
- **`render_api_docs.py`**: Script to validate and render API JSON to Markdown

## Usage

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

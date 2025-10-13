# Development Notes

> Local development conventions and commands for working on Currant

## Python Development

### Virtual Environment
- Python virtualenv is at: `python/.venv/`
- NOT at project root `.venv/`

### Building Python Bindings

```bash
# From project root
cd python
.venv/bin/maturin develop --release -q

# Or from python/ directory
.venv/bin/maturin develop --release -q
```

### Running Python Commands

```bash
# From project root
python/.venv/bin/python -m currant <command>

# From python/ directory
.venv/bin/python -m currant <command>
```

## Database

Default connection string:
```
postgresql://workflows:workflows@localhost/workflows
```

Started via:
```bash
docker compose up -d
```

## Common Development Tasks

### Run migrations
```bash
cd python
.venv/bin/python -m currant migrate
```

### Run worker
```bash
cd python
.venv/bin/python -m currant worker --queue default
```

### Run benchmark (after Step 2 refactoring)
```bash
cd python
.venv/bin/python -m currant bench --workers 10 --tasks 1000
```

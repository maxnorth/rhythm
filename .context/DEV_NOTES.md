# Development Notes

> Local development conventions and commands for working on Rhythm

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
python/.venv/bin/python -m rhythm <command>

# From python/ directory
.venv/bin/python -m rhythm <command>
```

## Database

Default connection string:
```
postgresql://rhythm@localhost/rhythm
```

Started via:
```bash
docker compose up -d
```

### Reset Database

When asked to "reset the database", use this command:
```bash
docker compose down -v && docker compose up -d
```

This drops the postgres volume (wiping all data) and restarts with a fresh database.
After resetting, you must run migrations again before using the database.

## Common Development Tasks

### Run migrations
```bash
cd python
.venv/bin/python -m rhythm --database-url postgresql://rhythm@localhost/rhythm migrate
```

### Run worker
```bash
cd python
.venv/bin/python -m rhythm --database-url postgresql://rhythm@localhost/rhythm worker --queue default
```

### Run benchmark
```bash
cd python
.venv/bin/python -m rhythm --database-url postgresql://rhythm@localhost/rhythm worker bench --workers 10 --tasks 1000
```

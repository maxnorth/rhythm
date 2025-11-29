# Publishing Scripts

Scripts for building and publishing the Rhythm Python package.

## publish.py

Build and publish the Rhythm Python package to PyPI.

### Prerequisites

Make sure you have `maturin` installed:

```bash
pip install maturin
```

For publishing, you'll need PyPI credentials. Set them up using one of these methods:

1. **API Token (recommended)**: Create a token at https://pypi.org/manage/account/token/
   ```bash
   export MATURIN_PYPI_TOKEN="your-token-here"
   ```

2. **Username/Password**: The script will prompt you

### Usage Examples

#### Build only (no publish)

```bash
python scripts/publish.py --build-only
```

Built wheels will be in the `dist/` directory.

#### Build and publish to TestPyPI

```bash
python scripts/publish.py --test
```

Useful for testing the package before publishing to production PyPI.

#### Bump patch version and publish to PyPI

```bash
# Bump patch version (e.g., 0.1.0 -> 0.1.1)
python scripts/publish.py --bump-version patch --publish

# Bump minor version (e.g., 0.1.0 -> 0.2.0)
python scripts/publish.py --bump-version minor --publish

# Bump major version (e.g., 0.1.0 -> 1.0.0)
python scripts/publish.py --bump-version major --publish
```

#### Set specific version and publish

```bash
python scripts/publish.py --version 0.2.0 --publish
```

#### Create git tag after building

```bash
python scripts/publish.py --bump-version patch --publish --tag
```

This will:
1. Bump the version
2. Build the package
3. Publish to PyPI
4. Create and optionally push a git tag

### Development Builds

For development builds (non-optimized):

```bash
python scripts/publish.py --dev --build-only
```

### Command-line Options

- `--build-only`: Only build the package, don't publish
- `--test`: Publish to TestPyPI instead of PyPI
- `--publish`: Publish to PyPI (production)
- `--version VERSION`: Set a specific version (e.g., 0.2.0)
- `--bump-version {major,minor,patch}`: Bump the version before building
- `--tag`: Create a git tag for the version
- `--dev`: Build in development mode (not release)
- `--no-clean`: Don't clean build artifacts before building

### Full Release Workflow

```bash
# 1. Make sure all changes are committed
git status

# 2. Bump version, build, publish, and tag
python scripts/publish.py --bump-version patch --publish --tag

# 3. Push the tag (if you didn't do it via the script)
git push origin v0.1.1
```

## Notes

- The script automatically updates the version in `pyproject.toml`
- Build artifacts are cleaned before each build (unless `--no-clean` is used)
- The script uses `maturin` for building and publishing
- For TestPyPI, you'll need separate credentials from https://test.pypi.org/

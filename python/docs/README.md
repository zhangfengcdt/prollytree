# ProllyTree Python Documentation

This directory contains the Sphinx documentation for ProllyTree Python bindings.

## Local Development

### Prerequisites

```bash
pip install sphinx sphinx_rtd_theme sphinx-autodoc-typehints myst-parser maturin
```

### Building Documentation Locally

```bash
# Build Python bindings and documentation
./build_docs.sh

# Or build documentation only (requires prollytree to be installed)
sphinx-build -b html . _build/html
```

### Viewing Documentation

```bash
# Open in browser
open _build/html/index.html

# Or serve locally
cd _build/html && python -m http.server 8000
# Then visit: http://localhost:8000
```

## Read the Docs Integration

This documentation is configured to be built automatically on Read the Docs using the `.readthedocs.yaml` file in the project root.

### File Structure

- `conf.py` - Sphinx configuration
- `index.rst` - Main documentation page
- `quickstart.rst` - Getting started guide
- `api.rst` - Auto-generated API reference
- `examples.rst` - Comprehensive examples
- `advanced.rst` - Advanced usage patterns
- `requirements.txt` - Documentation dependencies
- `build_docs.sh` - Local build script

### Auto-Generated Content

The API documentation is automatically generated from the Python bindings using Sphinx autodoc. This includes:

- ProllyTree class and methods
- VersionedKvStore for Git-like version control
- ProllySQLStore for SQL query support
- AgentMemorySystem for AI agent memory
- All supporting classes and enums

### Adding New Documentation

1. Add new `.rst` files to this directory
2. Update `index.rst` to include them in the toctree
3. Rebuild documentation with `./build_docs.sh`

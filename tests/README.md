# ProllyTree Integration Tests

This directory contains integration tests that validate ProllyTree functionality beyond the unit tests.

## Test Files

This directory is for integration tests that complement the unit tests in `src/`.

## Test Organization

- **Unit Tests**: Located in `src/` files using `#[cfg(test)]` modules
- **Integration Tests**: Located in this `tests/` directory
- **Python Tests**: Located in `python/tests/` directory
- **Example Usage**: Located in `python/examples/` directory

## Adding New Tests

When adding new integration tests:
1. Follow the naming convention `test_[feature]_integration.py`
2. Include comprehensive documentation
3. Ensure tests are self-contained and don't depend on external state
4. Add appropriate error handling and cleanup
5. Update this README with the new test description

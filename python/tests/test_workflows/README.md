# Test Workflows

This directory contains workflow definitions and task functions for testing the Rhythm workflow engine.

## Structure

```
test_workflows/
├── README.md                          # This file
├── __init__.py                        # Package marker
├── tasks.py                           # Shared task functions
├── complex_expressions.flow           # Workflow: Multiple operations with property access
├── deeply_nested_properties.flow      # Workflow: Deeply nested property access
├── empty_object.flow                  # Workflow: Empty object handling
├── literal_values.flow                # Workflow: Literal values in arguments
├── mixed_inputs_and_results.flow      # Workflow: Mixing inputs and task results
├── multiple_property_chains.flow      # Workflow: Multiple property accesses
├── no_tasks.flow                      # Workflow: Immediate return without tasks
├── object_construction.flow           # Workflow: Building objects from results
├── property_access.flow               # Workflow: Nested property access
├── return_literal.flow                # Workflow: Literal return value
├── sequential_tasks.flow              # Workflow: Sequential task execution
└── single_task.flow                   # Workflow: Single task execution
```

## Workflows

Each `.flow` file defines a workflow that tests specific DSL functionality:

### sequential_tasks.flow
Tests simple sequential execution of multiple tasks, verifying that:
- Tasks execute in order
- Results from previous tasks can be used as inputs
- State is maintained across task executions

### property_access.flow
Tests accessing nested properties from task results:
- `user.data.name` - accessing nested object properties
- Property chains work as expected in function arguments

### complex_expressions.flow
Tests multiple operations with property access:
- Multiple task executions
- Property access combined with arithmetic
- Chaining operations: `(a + b) * 2`

### object_construction.flow
Tests building complex objects from multiple task results:
- Combining results from different tasks
- Mixing task results with workflow inputs
- Object construction with named properties

### no_tasks.flow
Tests workflows that return immediately without executing any tasks:
- Immediate return values
- Workflow inputs passed through to output

### single_task.flow
Tests the simplest case of executing a single task:
- Basic task execution
- Result pass-through

### deeply_nested_properties.flow
Tests accessing deeply nested properties:
- `data.level1.level2.level3.value` - four levels deep
- Property chain resolution

### multiple_property_chains.flow
Tests multiple property accesses in a single task call:
- Accessing properties from different task results
- Multiple property chains in same object literal

### literal_values.flow
Tests using literal values in task arguments:
- Numbers: `123`, `99.5`
- Strings: `"test"`
- Booleans: `true`, `false`

### mixed_inputs_and_results.flow
Tests mixing workflow inputs and task results:
- Using both `inputs.x` and `taskResult.y` in same expression
- Combining inputs from different sources

### empty_object.flow
Tests passing empty objects `{}` to tasks:
- Empty object handling
- Default value resolution

### return_literal.flow
Tests returning literal objects without task execution:
- Object literal returns
- No task dependencies

## Tasks

All task functions are defined in `tasks.py`:

- `increment(args)` - Increment a number by 1
- `add(args)` - Add two numbers
- `multiply(args)` - Multiply two numbers
- `create_user(args)` - Create a user object
- `greet_user(args)` - Format a greeting message
- `get_number(args)` - Get a predefined number
- `get_first_name(args)` - Return "John"
- `get_last_name(args)` - Return "Doe"
- `format_name(args)` - Format a full name
- `echo(args)` - Echo back a message
- `get_nested_data(args)` - Return deeply nested structure
- `process_value(args)` - Process a value (multiply by 2)
- `get_metadata(args)` - Return metadata object
- `combine_data(args)` - Combine multiple fields into string
- `create_record(args)` - Create a record with given fields
- `get_defaults(args)` - Return default configuration values

## Usage

The test suite automatically discovers all `.flow` files in this directory and registers them as workflows. Tasks are imported from `tasks.py`.

To run the tests:

```bash
pytest python/tests/test_workflow_suite.py -v
```

## Adding New Tests

1. Create a new `.flow` file with your workflow definition
2. Add any required task functions to `tasks.py`
3. Add a test function in `test_workflow_suite.py` that:
   - Starts the workflow with `RustBridge.start_workflow()`
   - Waits for completion with `wait_for_workflow_completion()`
   - Asserts on the expected result

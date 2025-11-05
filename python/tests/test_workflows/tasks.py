"""Shared task functions for workflow tests"""

import rhythm


@rhythm.task(queue="default")
async def increment(args):
    """Increment a number by 1"""
    return {"result": args["value"] + 1}


@rhythm.task(queue="default")
async def create_user(args):
    """Create a user object"""
    return {
        "data": {
            "name": args["name"],
            "age": args["age"],
            "id": 123
        }
    }


@rhythm.task(queue="default")
async def greet_user(args):
    """Greet a user"""
    return {
        "greeting": f"Hello {args['user_name']}, age {args['user_age']}!"
    }


@rhythm.task(queue="default")
async def get_number(args):
    """Get a predefined number"""
    numbers = {"a": 5, "b": 3, "c": 10}
    return {"value": numbers.get(args["key"], 0)}


@rhythm.task(queue="default")
async def add(args):
    """Add two numbers"""
    return {"result": args["a"] + args["b"]}


@rhythm.task(queue="default")
async def multiply(args):
    """Multiply two numbers"""
    return {"result": args["a"] * args["b"]}


@rhythm.task(queue="default")
async def get_first_name(args):
    """Get first name for user"""
    return {"name": "John"}


@rhythm.task(queue="default")
async def get_last_name(args):
    """Get last name for user"""
    return {"name": "Doe"}


@rhythm.task(queue="default")
async def format_name(args):
    """Format full name"""
    return {
        "full_name": f"{args['title']} {args['first']} {args['last']}"
    }


@rhythm.task(queue="default")
async def echo(args):
    """Echo back the message"""
    return {"message": args["message"]}


@rhythm.task(queue="default")
async def get_nested_data(args):
    """Return deeply nested data structure"""
    return {
        "level1": {
            "level2": {
                "level3": {
                    "value": 42
                }
            }
        }
    }


@rhythm.task(queue="default")
async def process_value(args):
    """Process a value"""
    return {"processed": args["val"] * 2}


@rhythm.task(queue="default")
async def get_metadata(args):
    """Get metadata"""
    return {
        "info": {
            "timestamp": 1234567890,
            "version": "1.0.0"
        }
    }


@rhythm.task(queue="default")
async def combine_data(args):
    """Combine multiple data fields"""
    return {
        "combined": f"{args['name']} (age {args['age']}) - v{args['version']} @ {args['timestamp']}"
    }


@rhythm.task(queue="default")
async def create_record(args):
    """Create a record with given fields"""
    return {
        "record": {
            "id": args["id"],
            "name": args["name"],
            "active": args["active"],
            "score": args["score"]
        }
    }


@rhythm.task(queue="default")
async def get_defaults(args):
    """Get default values"""
    return {
        "defaults": {
            "timeout": 30,
            "retries": 3
        }
    }


@rhythm.task(queue="default")
async def process_item(args):
    """Process an item by doubling it"""
    item = args["item"]
    return {"value": item * 2, "original": item}

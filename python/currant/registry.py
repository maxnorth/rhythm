"""Function registry for looking up decorated functions"""

from typing import Callable, Dict

# Global registry of function_name -> function
_FUNCTION_REGISTRY: Dict[str, Callable] = {}


def register_function(name: str, fn: Callable):
    """Register a function in the global registry"""
    _FUNCTION_REGISTRY[name] = fn


def get_function(name: str) -> Callable:
    """Get a function from the registry"""
    if name not in _FUNCTION_REGISTRY:
        raise ValueError(f"Function '{name}' not found in registry. Did you import it?")
    return _FUNCTION_REGISTRY[name]


def clear_registry():
    """Clear the function registry (useful for testing)"""
    _FUNCTION_REGISTRY.clear()

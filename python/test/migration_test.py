#!/usr/bin/env python3
"""Simple test to verify initialization and migrations are working"""
import sys
sys.path.insert(0, "python")

from rhythm.rust_bridge import RustBridge

print("=" * 60)
print("Testing RustBridge.initialize() with auto_migrate=True")
print("=" * 60)

print("\n1. Calling initialize()...")
RustBridge.initialize(
    database_url="postgresql://rhythm@localhost/rhythm",
    auto_migrate=True,
    workflows=[]
)
print("2. initialize() returned successfully\n")

print("=" * 60)
print("Test completed")
print("=" * 60)

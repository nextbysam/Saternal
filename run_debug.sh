#!/bin/bash

# Run Saternal with DEBUG logging enabled
# This will show all the 🔍 debug logs we added

echo "Starting Saternal with DEBUG logging..."
echo "Look for 🔍 markers in the output"
echo ""

# Enable debug logs for all saternal modules
RUST_LOG=debug ./target/release/saternal

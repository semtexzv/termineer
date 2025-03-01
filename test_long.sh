#!/bin/bash
echo "Starting test script that runs for a while..."
echo "You can press Ctrl+C to interrupt this script."
echo "The terminal mode handling has been improved to avoid race conditions."
echo "==============================================" 

for i in {1..30}; do
  echo "Processing iteration $i of 30..."
  echo "Simulating work: $(date)"
  echo "Progress: $((i * 100 / 30))%"
  echo "------------------------------------------"
  sleep 1
done

echo "Script completed successfully."
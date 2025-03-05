#!/bin/bash
set -e

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}======= AutoSWE Client-Server Integration Test =======${NC}"

# Check if server is already running on port 3000
if nc -z localhost 3000 2>/dev/null; then
    echo -e "${BLUE}Server is already running on port 3000${NC}"
else
    echo -e "${BLUE}Starting server on port 3000...${NC}"
    # Start the server in a new terminal window and background it
    if [[ "$OSTYPE" == "darwin"* ]]; then
        # macOS
        osascript -e 'tell app "Terminal" to do script "cd '$(pwd)'/server && cargo run"' &
    else
        # Linux/Unix
        x-terminal-emulator -e "cd $(pwd)/server && cargo run" &
    fi
    
    # Wait for server to start
    echo "Waiting for server to start..."
    for i in {1..30}; do
        if nc -z localhost 3000 2>/dev/null; then
            echo -e "${GREEN}Server is up and running!${NC}"
            break
        fi
        if [ $i -eq 30 ]; then
            echo -e "${RED}Timed out waiting for server to start${NC}"
            exit 1
        fi
        sleep 1
        echo -n "."
    done
    echo ""
fi

# Test server reachability
echo -e "${BLUE}Testing server health endpoint...${NC}"
HEALTH_RESPONSE=$(curl -s http://localhost:3000/health)
if [ "$HEALTH_RESPONSE" = "OK" ]; then
    echo -e "${GREEN}Server health check passed: $HEALTH_RESPONSE${NC}"
else
    echo -e "${RED}Server health check failed: $HEALTH_RESPONSE${NC}"
    exit 1
fi

# Test list of valid test licenses
echo -e "${BLUE}Valid test license keys for testing:${NC}"
echo -e "${GREEN}TEST-DEV-LICENSE-KEY${NC} - Valid developer license"
echo -e "${GREEN}TEST-PRO-LICENSE-KEY${NC} - Valid professional license"
echo -e "${RED}TEST-EXPIRED-LICENSE${NC} - Expired license (should fail)"
echo -e "${GREEN}DEV-*${NC} - Any key starting with DEV- is valid for development"

echo ""
echo -e "${BLUE}Running client with valid license...${NC}"
# Set environment variables for the client
export AUTOSWE_SERVER_URL="http://localhost:3000"
export AUTOSWE_LICENSE_KEY="TEST-DEV-LICENSE-KEY"

# Run client with valid license in non-interactive mode
cargo run -- --model claude-3-haiku-20240307 "Hello world, what's your name?"

echo ""
echo -e "${BLUE}Running client with expired license (should fail)...${NC}"
export AUTOSWE_LICENSE_KEY="TEST-EXPIRED-LICENSE"

# Run client with expired license (should fail)
if cargo run -- --model claude-3-haiku-20240307 "This should fail" 2>/dev/null; then
    echo -e "${RED}Error: Client accepted expired license when it should have failed${NC}"
    exit 1
else
    echo -e "${GREEN}Success! Client correctly rejected expired license${NC}"
fi

echo ""
echo -e "${BLUE}Testing invalid license format (should fail)...${NC}"
export AUTOSWE_LICENSE_KEY="INVALID-KEY-FORMAT"

# Run client with invalid license (should fail)
if cargo run -- --model claude-3-haiku-20240307 "This should fail" 2>/dev/null; then
    echo -e "${RED}Error: Client accepted invalid license when it should have failed${NC}"
    exit 1
else
    echo -e "${GREEN}Success! Client correctly rejected invalid license${NC}"
fi

echo ""
echo -e "${BLUE}Testing with development mode option...${NC}"
export AUTOSWE_SKIP_LICENSE="true"

# Run client with license check skipped
cargo run -- --model claude-3-haiku-20240307 "This should work with license check skipped"

echo ""
echo -e "${GREEN}===== All integration tests passed! =====${NC}"
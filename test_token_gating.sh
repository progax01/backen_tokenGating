#!/bin/bash

# Token Gating Backend Test Script
# This script starts the token gating backend and tests the complete API flow

# Set variables
SERVER_PORT=8090
BACKEND_BIN="./target/release/token-gating-backend"
RESOURCE_NAME="premium-content"
MINT_ADDRESS="TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
USER_WALLET="3xHGQhGarP88kcTNRBDtXgAtfbfWHvHPsaxNhbKzJv19" # Example wallet, replace with actual test wallet
LOG_FILE="test_flow.log"

# Function to check if a command succeeds
check_success() {
  if [ $? -eq 0 ]; then
    echo "✅ $1"
  else
    echo "❌ $1"
    echo "Exiting due to error..."
    exit 1
  fi
}

# Function to make API calls with proper logging
call_api() {
  local method=$1
  local endpoint=$2
  local data=$3
  local description=$4
  
  echo -e "\n⏳ $description..."
  
  if [ -z "$data" ]; then
    # GET request
    response=$(curl -s -X $method "http://localhost:$SERVER_PORT$endpoint")
  else
    # POST request with data
    response=$(curl -s -X $method "http://localhost:$SERVER_PORT$endpoint" \
               -H "Content-Type: application/json" \
               -d "$data")
  fi
  
  echo "$response" | jq . || echo "$response"
  echo "$description: $response" >> $LOG_FILE
  
  # Check if response contains error
  if echo "$response" | grep -q "error"; then
    echo "❌ $description"
    return 1
  else
    echo "✅ $description"
    return 0
  fi
}

# Create a new log file
echo "Token Gating Backend Test Flow - $(date)" > $LOG_FILE

# Check if the backend binary exists
if [ ! -f "$BACKEND_BIN" ]; then
  echo "❌ Backend binary not found at $BACKEND_BIN"
  echo "Building the backend..."
  cargo build --release
  check_success "Build backend"
fi

# Start the backend server in the background
echo "🚀 Starting Token Gating Backend on port $SERVER_PORT..."
RUST_LOG=info $BACKEND_BIN -p $SERVER_PORT > backend.log 2>&1 &
SERVER_PID=$!

# Wait for the server to start
echo "⏳ Waiting for server to start..."
sleep 3

# Check if server is running
if ! ps -p $SERVER_PID > /dev/null; then
  echo "❌ Server failed to start. Check backend.log for details."
  exit 1
fi

echo "✅ Server started with PID $SERVER_PID"

# Test health endpoint
call_api "GET" "/health" "" "Health check"

# 1. Configure a resource
resource_data='{
  "name": "'$RESOURCE_NAME'",
  "required_mint": "'$MINT_ADDRESS'",
  "level_thresholds": [10, 50, 100]
}'
call_api "POST" "/admin/configure-resource" "$resource_data" "Configure resource"

# 2. Get resource requirements
call_api "GET" "/resource/requirements/$RESOURCE_NAME" "" "Get resource requirements"

# 3. Verify access for a user
call_api "GET" "/access/verify/$USER_WALLET/$RESOURCE_NAME" "" "Verify user access"

# 4. Configure claim window
current_time=$(date +%s)
end_time=$((current_time + 604800)) # One week from now
claim_data='{
  "resource_name": "'$RESOURCE_NAME'",
  "start_time": '$current_time',
  "end_time": '$end_time',
  "required_level": 1
}'
call_api "POST" "/admin/configure-claim" "$claim_data" "Configure claim window"

# 5. Add a new admin (optional)
new_admin_data='{
  "resource_name": "'$RESOURCE_NAME'",
  "admin_pubkey": "8zGxf1nS4oqXVwQYCu834tZ2s4j1cuqxZmUDXdJope5e"
}'
call_api "POST" "/admin/add" "$new_admin_data" "Add new admin"

# Show summary
echo -e "\n📋 Test Flow Summary:"
echo "------------------------"
echo "Resource name: $RESOURCE_NAME"
echo "Token mint address: $MINT_ADDRESS"
echo "Test wallet address: $USER_WALLET"
echo "Server running on port: $SERVER_PORT with PID: $SERVER_PID"
echo "Log file: $LOG_FILE"
echo "------------------------"

# Ask if the user wants to stop the server
read -p "Do you want to stop the backend server? (y/n): " stop_server
if [ "$stop_server" = "y" ] || [ "$stop_server" = "Y" ]; then
  echo "Stopping server with PID $SERVER_PID..."
  kill $SERVER_PID
  check_success "Server stopped"
else
  echo "Server is still running with PID $SERVER_PID"
  echo "To stop it manually, run: kill $SERVER_PID"
fi

echo "Test flow completed. See $LOG_FILE for details."
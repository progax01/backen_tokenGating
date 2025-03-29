# Token Gating Solana Program

## Overview

This Solana program implements a flexible token-based access control and reward system, enabling sophisticated resource management through token ownership and levels.

## Features

### 1. Resource Configuration
- Create and manage token-gated resources
- Specify required token mint for resource access
- Configure resource-specific settings

### 2. Access Control
- Verify user access based on token ownership
- Require minimum token balance for resource access
- Emit detailed access events (granted/denied)

### 3. Tiered Level System
- Define up to 3 token balance thresholds
- Dynamically calculate user access levels
- Support granular access control based on token holdings

### 4. Claim Windows
- Create time-limited token claim periods
- Set level requirements for claims
- Prevent multiple claims within the same window

### 5. Admin Management
- Multi-admin support (up to 3 administrators)
- Add/remove administrators securely
- Prevent removal of the last admin

## Core Functions

### Resource Management
- `configure_resource()`: Create a new token-gated resource
- `update_resource()`: Modify existing resource requirements
- `verify_access()`: Check user's access to a resource

### Levels and Claims
- `configure_levels()`: Set token balance thresholds
- `check_level()`: Determine user's access level
- `configure_claim()`: Create token claim windows
- `claim_token()`: Allow users to claim tokens

### Admin Control
- `add_admin()`: Add new administrators
- `remove_admin()`: Remove administrators

## Error Handling

Comprehensive error codes cover scenarios like:
- Insufficient token balance
- Invalid resource configuration
- Access permission issues
- Claim window violations

## Events

The program emits detailed events for:
- Resource configuration
- Access attempts
- Level changes
- Token claims
- Admin modifications

## Security Considerations
- Resource names limited to 32 characters
- Strict access control mechanisms
- Prevents unauthorized resource modifications

## Requirements
- Solana blockchain
- Anchor framework
- SPL Token program

## License
MIT License - see LICENSE file for details

## Contributing
Contributions welcome! Please read the contributing guidelines before submitting pull requests. 
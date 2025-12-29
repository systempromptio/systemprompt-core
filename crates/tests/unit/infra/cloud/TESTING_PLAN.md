# systemprompt-cloud Unit Tests

## Crate Overview
Cloud infrastructure, OAuth, and API client for external SystemPrompt Cloud. Handles OAuth flows, checkout, credentials, and tenant management.

## Source Files
- `src/api_client/` - Cloud API communication
- `src/checkout/` - Subscription checkout flow
- `src/context/` - Cloud context management
- `src/credentials/` - Credential handling
- `src/credentials_bootstrap/` - Credential setup
- `src/jwt/` - JWT handling
- `src/oauth/` - OAuth flow
- `src/paths/` - Path utilities
- `src/tenants/` - Tenant management

## Test Plan

### Cloud API Client Tests
- `test_api_client_initialization` - Client setup
- `test_api_client_auth_header` - Auth header injection
- `test_api_client_request_success` - Successful request
- `test_api_client_request_error` - Error handling

### OAuth Flow Tests
- `test_oauth_flow_initiation` - Start OAuth
- `test_oauth_flow_callback_handling` - Handle callback
- `test_oauth_flow_token_exchange` - Token exchange

### Credentials Tests
- `test_credentials_storage` - Store credentials
- `test_credentials_retrieval` - Retrieve credentials
- `test_credentials_refresh` - Refresh tokens
- `test_credentials_bootstrap` - Initial setup

### Checkout Tests
- `test_checkout_callback_flow` - Checkout callback
- `test_checkout_subscription_status` - Status checking

### Tenant Tests
- `test_tenant_store_save` - Save tenant
- `test_tenant_store_load` - Load tenant
- `test_tenant_info_parsing` - Parse tenant info

## Mocking Requirements
- Mock HTTP client
- Mock filesystem for credentials
- Mock OAuth server

## Test Fixtures Needed
- Sample OAuth responses
- Sample credential files
- Sample tenant data

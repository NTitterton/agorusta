# E2E Tests for Agorusta

End-to-end tests using Playwright.

## Prerequisites

- Node.js 18+
- Frontend dev server running on http://localhost:5173
- Backend deployed to AWS (tests use the real API)

## Setup

```bash
cd e2e-tests
npm install
npx playwright install chromium
```

## Running Tests

```bash
# Run all tests (will start frontend dev server automatically)
npm test

# Run tests with UI (interactive mode)
npm run test:ui

# Run tests in headed browser (see the browser)
npm run test:headed

# Run specific test file
npm run test:auth
npm run test:presence
npm run test:messaging

# Debug mode
npm run test:debug

# View test report
npm run report
```

## Test Structure

- `tests/auth.spec.ts` - Authentication tests (register, login)
- `tests/presence.spec.ts` - User presence indicators (online/offline status)
- `tests/messaging.spec.ts` - Message sending and history
- `tests/helpers.ts` - Common test utilities
- `tests/fixtures.ts` - Test data

## Notes

- Tests create unique users/servers per run to avoid conflicts
- Presence tests use two browser contexts to simulate two users
- The frontend dev server is started automatically by Playwright

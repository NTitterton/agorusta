# CLAUDE.md

This file provides guidance to Claude Code when working with this repository.

## Project Overview

Agorusta is a Discord-like chat application with a Rust serverless backend and SvelteKit frontend, optimized for solo dev hosting costs on AWS.

## Current Status

The core features are implemented and deployed:
- User authentication (JWT with Argon2 password hashing)
- Server creation with unique names
- Server joining via invite codes or name+password
- Text channels with real-time messaging
- Direct messages between users
- WebSocket-powered live updates

## Architecture

- **Frontend**: SvelteKit 5 with TypeScript, using Svelte runes ($state, $effect, $derived)
- **Backend**: Rust Lambda functions (api + websocket)
- **Database**: DynamoDB (10 tables - see project_design.md)
- **Real-time**: API Gateway WebSockets
- **IaC**: AWS SAM (template.yaml)

## Key Files

| File | Purpose |
|------|---------|
| `template.yaml` | AWS SAM infrastructure definition |
| `backend/lambdas/api/src/main.rs` | API routes and handlers |
| `backend/lambdas/api/src/dms.rs` | Direct messages logic |
| `backend/lambdas/api/src/invites.rs` | Invite codes and server passwords |
| `frontend/src/lib/api.ts` | Frontend API client and types |
| `frontend/src/lib/websocket.svelte.ts` | WebSocket service |

## Common Commands

```bash
# Build and deploy backend
sam build && sam deploy

# Run frontend dev server
cd frontend && npm run dev

# Check backend compilation
cd backend && cargo check
```

## Development Notes

- Backend timestamps are stored in milliseconds (not seconds)
- Conversation IDs for DMs are deterministic: `min(user1, user2)_max(user1, user2)`
- WebSocket subscriptions use channel_id for both channels and DM conversations
- Server names must be unique (enforced via GSI query)

## Potential Future Work

- File uploads (S3)
- Voice channels (WebRTC)
- Server roles and permissions
- Message editing/deletion
- Typing indicators
- Read receipts

# Agorusta

A Discord-like chat application built with Rust serverless backend and SvelteKit frontend, optimized for solo dev hosting costs.

## Features

- User authentication (JWT with Argon2 hashing)
- Create and join servers (invite codes or name+password)
- Text channels with real-time messaging
- Direct messages between users
- User presence (online/offline indicators)
- Members sidebar with role grouping (Owner, Admin, Member)
- WebSocket-powered live updates
- File uploads (images, documents)
- Typing indicators

## Tech Stack

| Layer | Technology |
|-------|------------|
| Frontend | SvelteKit 5, TypeScript |
| Backend | Rust, AWS Lambda |
| Database | DynamoDB |
| Real-time | API Gateway WebSockets |
| Storage | S3 |
| IaC | AWS SAM |

## Quick Start

### Prerequisites

- [Rust](https://rustup.rs/) (1.88+)
- [Node.js](https://nodejs.org/) (22+)
- [AWS CLI](https://aws.amazon.com/cli/) configured
- [AWS SAM CLI](https://docs.aws.amazon.com/serverless-application-model/latest/developerguide/install-sam-cli.html)
- [cargo-lambda](https://www.cargo-lambda.info/)

### Development

```bash
# Backend
cd backend && cargo check

# Frontend
cd frontend && npm install && npm run dev
```

### Deploy

```bash
sam build && sam deploy
```

### Testing

```bash
# Backend unit tests
cd backend && cargo test

# E2E tests (requires frontend running)
cd e2e-tests && npm install
npx playwright install chromium
npm test
```

## Documentation

See [project_design.md](project_design.md) for detailed architecture and design documentation.

## License

MIT

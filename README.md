# Agorusta

A Discord-like chat application built with Rust serverless backend and SvelteKit frontend, optimized for solo dev hosting costs.

## Features

- User authentication (JWT)
- Create and join servers (invite codes or name+password)
- Text channels with real-time messaging
- Direct messages between users
- WebSocket-powered live updates

## Tech Stack

| Layer | Technology |
|-------|------------|
| Frontend | SvelteKit, TypeScript |
| Backend | Rust, AWS Lambda |
| Database | DynamoDB |
| Real-time | API Gateway WebSockets |
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

## Documentation

See [project_design.md](project_design.md) for detailed architecture and design documentation.

## License

MIT

# Agorusta

A Discord-like chat application built with Rust serverless backend and SvelteKit frontend, optimized for solo dev hosting costs.

## Architecture

```
┌─────────────────┐     ┌──────────────────────────────────────────┐
│                 │     │              AWS                         │
│   SvelteKit     │────▶│  ┌─────────────┐    ┌─────────────────┐  │
│   Frontend      │     │  │ API Gateway │───▶│  Rust Lambda    │  │
│                 │     │  │   (REST)    │    │  (api)          │  │
└─────────────────┘     │  └─────────────┘    └────────┬────────┘  │
                        │                              │           │
                        │  ┌─────────────┐    ┌────────▼────────┐  │
                        │  │ API Gateway │───▶│    DynamoDB     │  │
                        │  │ (WebSocket) │    │                 │  │
                        │  └──────┬──────┘    └─────────────────┘  │
                        │         │                                │
                        │  ┌──────▼──────┐                         │
                        │  │ Rust Lambda │                         │
                        │  │ (websocket) │                         │
                        │  └─────────────┘                         │
                        └──────────────────────────────────────────┘
```

### Why Serverless?

- **Pay per use** - No idle server costs
- **Auto-scaling** - Handles traffic spikes without configuration
- **Zero ops** - No servers to patch or maintain

### Tech Stack

| Layer | Technology |
|-------|------------|
| Frontend | SvelteKit, TypeScript |
| Backend | Rust, Lambda, API Gateway |
| Database | DynamoDB |
| Real-time | API Gateway WebSockets |
| IaC | AWS SAM |

## Project Structure

```
agorusta/
├── backend/
│   ├── lambdas/
│   │   ├── api/           # REST API Lambda
│   │   └── websocket/     # WebSocket Lambda
│   ├── shared/            # Shared types and utilities
│   └── Cargo.toml         # Rust workspace
├── frontend/              # SvelteKit app
├── template.yaml          # AWS SAM template
└── samconfig.toml         # SAM deployment config
```

## Prerequisites

- [Rust](https://rustup.rs/) (1.88+)
- [Node.js](https://nodejs.org/) (22.12+)
- [AWS CLI](https://aws.amazon.com/cli/) configured with credentials
- [AWS SAM CLI](https://docs.aws.amazon.com/serverless-application-model/latest/developerguide/install-sam-cli.html)
- [cargo-lambda](https://www.cargo-lambda.info/)

```bash
# Install cargo-lambda
cargo install cargo-lambda

# Install SAM CLI (macOS)
brew install aws-sam-cli
```

## Development

### Backend

```bash
cd backend

# Check compilation
cargo check

# Run tests
cargo test

# Local Lambda development (with hot reload)
cargo lambda watch
```

### Frontend

```bash
cd frontend

# Install dependencies
npm install

# Start dev server
npm run dev
```

### Local Testing

```bash
# In one terminal - run the Lambda locally
cd backend
cargo lambda watch

# In another terminal - run the frontend
cd frontend
npm run dev
```

## Deployment

### Build

```bash
# Build all Lambda functions
sam build
```

### Deploy

```bash
# First time (interactive)
sam deploy --guided

# Subsequent deploys
sam deploy
```

### Outputs

After deployment, SAM will output:
- **HttpApiUrl** - REST API endpoint
- **WebSocketUrl** - WebSocket endpoint for real-time messaging

## DynamoDB Tables

| Table | Primary Key | Sort Key | Purpose |
|-------|-------------|----------|---------|
| Users | id (S) | - | User accounts |
| Servers | id (S) | - | Discord-like servers |
| Messages | channel_id (S) | created_at (N) | Chat messages |
| Connections | connection_id (S) | - | WebSocket connections |

## Environment Variables

Lambda functions receive these environment variables:

| Variable | Description |
|----------|-------------|
| RUST_LOG | Log level (info, debug, etc.) |
| USERS_TABLE | Users DynamoDB table name |
| SERVERS_TABLE | Servers DynamoDB table name |
| MESSAGES_TABLE | Messages DynamoDB table name |
| CONNECTIONS_TABLE | WebSocket connections table |
| WEBSOCKET_ENDPOINT | WebSocket API endpoint (for sending messages) |

## Roadmap

- [ ] User authentication (JWT)
- [ ] Create/join servers
- [ ] Text channels
- [ ] Real-time messaging
- [ ] File uploads (S3)
- [ ] Voice channels (WebRTC)

## Cost Estimate

For a solo dev or small user base:

| Service | Free Tier | Cost After |
|---------|-----------|------------|
| Lambda | 1M requests/month | $0.20/1M |
| API Gateway | 1M REST calls/month | $1/1M |
| DynamoDB | 25 GB storage | $0.25/GB |
| WebSocket | 1M messages | $1/1M |

**Estimated monthly cost for light usage: $0-5**

## License

MIT

# Fly.io Deployment Guide for AutoSWE Server

Fly.io is a platform that deploys applications close to users, making it ideal for applications requiring global presence. It has excellent support for Rust applications and offers PostgreSQL databases with reasonable pricing.

## Overview

- **Platform Type**: Global edge deployment platform
- **Deployment Method**: Docker containers
- **Database**: Managed PostgreSQL (Fly Postgres)
- **Scaling**: Horizontal and vertical scaling options
- **Pricing Model**: Pay-as-you-go with free tier available

## Pricing (as of 2024)

### Compute Resources
- **Free Tier**: 3 shared-CPU VMs with 256MB RAM
- **Production VM Options**:
  - Shared CPU, 256MB RAM: $1.94/month ($0.0027/hour)
  - Shared CPU, 512MB RAM: $3.89/month ($0.0054/hour)
  - Dedicated CPU, 1GB RAM: $8.64/month ($0.012/hour)
  - Dedicated CPU, 2GB RAM: $17.28/month ($0.024/hour)
  - Larger options available

### PostgreSQL Database
- **Basic Postgres**: Starts at $15/month (1GB RAM, 10GB storage)
- **LiteFS Cloud**: $5/month for up to 10GB of database storage
- **Additional Storage**: $0.50/GB per month above 10GB

### Other Costs
- **Bandwidth**: 160GB free, then $0.02/GB outbound
- **IPv4 Addresses**: First 1 free, then $2/month
- **Persistent Volumes**: $0.15/GB per month

## Pros and Cons

### Pros
- Global edge deployment for low-latency worldwide
- Excellent Rust application support
- Simple deployment process with `fly.toml` configuration
- Built-in PostgreSQL offering
- WebSocket support
- Automatic HTTPS certificate management
- Free tier available

### Cons
- Can get expensive at scale
- Less comprehensive UI compared to some alternatives
- Database costs are higher than some competitors
- Limited control over infrastructure details

## Setup Instructions

1. **Install the Fly CLI**:
   ```bash
   curl -L https://fly.io/install.sh | sh
   ```

2. **Log in**:
   ```bash
   fly auth login
   ```

3. **Initialize a Fly App for AutoSWE Server**:
   ```bash
   # In your project directory
   fly launch
   ```

4. **Configure PostgreSQL Database**:
   ```bash
   fly postgres create --name autoswe-db
   ```

5. **Set Up Environment Variables**:
   ```bash
   fly secrets set DATABASE_URL="postgres://postgres:password@autoswe-db.internal:5432/postgres"
   fly secrets set JWT_SECRET="your_secure_jwt_secret"
   fly secrets set GOOGLE_CLIENT_ID="your_google_client_id"
   fly secrets set GOOGLE_CLIENT_SECRET="your_google_client_secret"
   fly secrets set OAUTH_REDIRECT_URL="https://your-app.fly.dev/auth/google/callback"
   ```

6. **Deploy the Application**:
   ```bash
   fly deploy
   ```

7. **Configure a Custom Domain (Optional)**:
   ```bash
   fly certs create your-domain.com
   ```

## Sample `fly.toml` Configuration

```toml
app = "autoswe-server"
primary_region = "sea"

[http_service]
  internal_port = 8080
  force_https = true
  auto_stop_machines = false
  auto_start_machines = true
  min_machines_running = 1
  processes = ["app"]

[env]
  PORT = "8080"

[experimental]
  allowed_public_ports = []
  auto_rollback = true

[[services]]
  http_checks = []
  internal_port = 8080
  processes = ["app"]
  protocol = "tcp"
  script_checks = []
  [services.concurrency]
    hard_limit = 25
    soft_limit = 20
    type = "connections"

  [[services.ports]]
    force_https = true
    handlers = ["http"]
    port = 80

  [[services.ports]]
    handlers = ["tls", "http"]
    port = 443
```

## Recommendation

Fly.io is highly recommended for the AutoSWE server because:

1. It provides excellent support for Rust applications
2. The global edge deployment is ideal for a user-facing application
3. It includes managed PostgreSQL for easy database setup
4. It handles HTTPS and domain configuration seamlessly
5. It supports WebSockets required for real-time interactions

This platform offers the best balance of simplicity, performance, and cost for most AutoSWE deployments. Start with the free tier for development, then scale to the appropriate paid resources based on user growth.
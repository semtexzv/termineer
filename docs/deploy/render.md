# Render.com Deployment Guide for AutoSWE Server

Render is a unified cloud platform that simplifies deployment with a user-friendly interface. It's an excellent option for deploying the AutoSWE server with minimal configuration and built-in PostgreSQL support.

## Overview

- **Platform Type**: Unified cloud platform
- **Deployment Method**: Git-based with automatic builds
- **Database**: Managed PostgreSQL
- **Scaling**: Vertical scaling with multiple instance types
- **Pricing Model**: Tiered pricing with free tier available

## Pricing (as of 2024)

### Web Services
- **Free Tier**: 
  - 512MB RAM, 0.1 CPU
  - 750 hours per month
  - Spins down after 15 minutes of inactivity
  - 100GB bandwidth

- **Starter**: $7/month
  - 512MB RAM, 0.5 CPU
  - Always on
  - 100GB bandwidth

- **Standard**: $15-$95/month
  - 1-4GB RAM, 1-2 CPU
  - Always on
  - 400GB-2TB bandwidth

### PostgreSQL Database
- **Free Tier**:
  - 1GB storage
  - 236MB RAM
  - Automatically suspends after 90 days

- **Starter**: $7/month
  - 1GB storage
  - Always on

- **Standard**: $20-$90/month
  - 2-8GB RAM
  - 10-64GB storage
  - HA options available at higher tiers

### Other Costs
- **Additional Bandwidth**: $0.10/GB
- **Build Minutes**: 500 free, then $0.003/minute
- **Additional Disk**: $0.10/GB per month

## Pros and Cons

### Pros
- Simple UI-based deployment process
- Automatic HTTPS certificates
- Easy integration with PostgreSQL
- Free tier for development
- Clean dashboard with logs and metrics
- Zero configuration needed for common frameworks
- Custom domains support

### Cons
- Free tier spins down after inactivity (not suitable for production)
- Limited global region options compared to Fly.io
- Fewer customization options than AWS
- Free database suspends after 90 days
- Slightly higher pricing at scale than some alternatives

## Setup Instructions

1. **Create a Render Account**:
   - Sign up at [render.com](https://render.com)

2. **Create a New Web Service**:
   - Connect your GitHub/GitLab repository
   - Select the repository with your AutoSWE server code

3. **Configure the Web Service**:
   - Name: `autoswe-server`
   - Environment: `Rust`
   - Build Command: `cargo build --release`
   - Start Command: `./target/release/autoswe-server`
   - Select appropriate instance type

4. **Create a PostgreSQL Database**:
   - Go to "New" > "PostgreSQL"
   - Name: `autoswe-db`
   - Choose appropriate database plan

5. **Set Up Environment Variables**:
   - In your web service settings, add:
     - `DATABASE_URL`: Use the Internal Database URL provided by Render
     - `JWT_SECRET`: Your secure JWT secret
     - `GOOGLE_CLIENT_ID`: Your Google client ID
     - `GOOGLE_CLIENT_SECRET`: Your Google client secret
     - `OAUTH_REDIRECT_URL`: `https://your-app.onrender.com/auth/google/callback`
     - `PORT`: 8080

6. **Configure Custom Domain (Optional)**:
   - In your web service settings, go to "Settings" > "Custom Domain"
   - Add your domain and follow the verification process

## Render-Specific Configuration

### render.yaml (Service Blueprint)

Create a `render.yaml` in your repository for automated setup:

```yaml
services:
  - type: web
    name: autoswe-server
    env: rust
    buildCommand: cargo build --release
    startCommand: ./target/release/autoswe-server
    envVars:
      - key: PORT
        value: 8080
      - key: JWT_SECRET
        generateValue: true
      - key: GOOGLE_CLIENT_ID
        sync: false
      - key: GOOGLE_CLIENT_SECRET
        sync: false
      - key: OAUTH_REDIRECT_URL
        sync: false
      - key: DATABASE_URL
        fromDatabase:
          name: autoswe-db
          property: connectionString

databases:
  - name: autoswe-db
    plan: starter
```

### Health Check Configuration

Add a health check endpoint to your Axum application and configure it in Render:

```rust
// In main.rs
.route("/health", get(health_check))

// Handler function
async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}
```

## Recommendation

Render is recommended for the AutoSWE server when:

1. You want the simplest deployment experience with minimal configuration
2. You're in the early stages and want to leverage the free tier for development
3. You need a straightforward PostgreSQL setup
4. You prefer a UI-based approach over command-line tools

The free tier is suitable for development, but for production, use at least the Starter plan for both web service and database to ensure they remain active. For higher traffic or larger datasets, scale to the Standard plans as needed.

Render represents an excellent balance of simplicity and functionality, making it a top choice for small to medium-sized applications. The straightforward deployment process significantly reduces DevOps overhead.
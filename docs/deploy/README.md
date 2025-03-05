# AutoSWE Server Deployment Options

This directory contains documentation for various deployment options for the AutoSWE server component. The server requires:

- A public-facing domain for OAuth callbacks and Stripe webhook handling
- PostgreSQL database
- Rust application hosting with WebSocket support

## Available Options

We've analyzed several cloud platforms suitable for hosting the AutoSWE server:

1. [Fly.io](fly-io.md) - Global edge deployment with strong Rust support
2. [Render](render.md) - Easy deployment with PostgreSQL integration
3. [Digital Ocean App Platform](digital-ocean.md) - Managed container deployment with PostgreSQL
4. [AWS](aws.md) - Comprehensive but complex deployment options
5. [Railway](railway.md) - Developer-friendly PaaS with usage-based pricing

## Comparison Summary

| Platform | Pros | Cons | Best For |
|----------|------|------|----------|
| Fly.io | Global edge deployment, excellent Rust support, simple deployment | Can get expensive at scale | Applications requiring global presence |
| Render | Simple UI, automatic HTTPS, easy PostgreSQL setup | Free tier spins down, limited regions | Quick deployment, small to medium scale |
| Digital Ocean | Simple deployment, good PostgreSQL integration | More expensive than alternatives | Projects requiring dedicated resources |
| AWS | Highly scalable, extensive features | Complex setup, more DevOps knowledge needed | Large-scale applications with complex requirements |
| Railway | Simple deployment, usage-based pricing | No free tier, potentially unpredictable costs | Small to medium projects with simple architecture |

See individual documentation files for detailed pricing, setup instructions, and recommendations.
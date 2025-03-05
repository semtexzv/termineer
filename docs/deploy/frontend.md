# Frontend Deployment Options for AutoSWE

This document outlines the recommended options for deploying the AutoSWE frontend website. While our backend server will be hosted on Fly.io, we have several specialized options for the frontend that provide better performance and cost-effectiveness than deploying it alongside the backend.

## Frontend Requirements

A well-deployed frontend for AutoSWE should have:

1. **Global CDN**: Fast content delivery worldwide
2. **Continuous Deployment**: Automated builds from Git
3. **Custom Domain Support**: With free SSL certificates
4. **Cost-Effectiveness**: Ideally with a free tier for static content
5. **Backend Integration**: Easy API communication with the Fly.io backend

## Recommended Options

### Primary Recommendation: Vercel

**Vercel** is our top recommendation for hosting the AutoSWE frontend website.

#### Vercel Highlights:
- **Pricing**: Free tier available for personal projects, $20/month for teams
- **Performance**: Excellent global CDN with edge caching
- **Deployment**: Seamless Git integration with preview deployments
- **Features**: 
  - Automatic HTTPS
  - Unlimited websites on the free tier
  - Analytics
  - Edge Functions for any server-side logic
  - Preview deployments for pull requests

#### Why Vercel with Fly.io Backend:
1. **Developer Experience**: Best-in-class developer experience with instant previews
2. **Optimized for Modern Frontends**: Native support for React, Vue, Angular, etc.
3. **Edge Network**: Global presence complements Fly.io's global backend
4. **Seamless CORS Setup**: Easy configuration for secure backend communication
5. **Free Tier Viability**: Can use the free tier until you reach scale

#### Setup Instructions:
1. Connect your GitHub/GitLab repository to Vercel
2. Configure build settings:
   ```
   Build Command: npm run build (or yarn build)
   Output Directory: build (or dist)
   ```
3. Configure environment variables:
   ```
   REACT_APP_API_URL=https://your-api.fly.dev (or your custom domain)
   ```
4. Set up a custom domain in the Vercel dashboard
5. Configure CORS on your Fly.io backend to allow requests from your Vercel domain

### Alternative 1: Netlify

**Netlify** is very similar to Vercel and makes an excellent alternative.

#### Netlify Highlights:
- **Pricing**: Free tier available, $19/month for Pro
- **Performance**: Global CDN with edge caching
- **Deployment**: Git integration with deploy previews
- **Features**:
  - Automatic HTTPS
  - Form handling
  - Serverless functions
  - Split testing

#### Why Consider Netlify:
- Better form handling capabilities than Vercel
- Slightly more generous bandwidth allowances on free tier
- Strong community and extensive plugin ecosystem

### Alternative 2: Cloudflare Pages

**Cloudflare Pages** offers exceptional performance with generous free tier limits.

#### Cloudflare Pages Highlights:
- **Pricing**: Very generous free tier, $20/month for Pro
- **Performance**: Unmatched global CDN via Cloudflare's network
- **Deployment**: Git integration with preview deployments
- **Features**:
  - Unlimited sites and requests
  - 500 builds per month on free tier
  - Workers integration for serverless functions
  - Analytics

#### Why Consider Cloudflare Pages:
- If you're already using Cloudflare for DNS
- Unlimited bandwidth on free tier
- Fastest global CDN performance

### Alternative 3: Render (Static Sites)

Since we already analyzed Render for backend deployment, it's worth noting they also offer excellent static site hosting.

#### Render Static Sites Highlights:
- **Pricing**: Free tier available
- **Performance**: Global CDN
- **Deployment**: Git integration
- **Features**:
  - Automatic HTTPS
  - Free custom domains
  - Global CDN

#### Why Consider Render:
- Simplifies vendor management by using same provider as a potential backend alternative
- Very straightforward deployment process
- Good free tier (100GB/month bandwidth)

## Cost Comparison

| Platform | Free Tier | Paid Tier | Bandwidth Allowance (Free) | Build Minutes (Free) |
|----------|-----------|-----------|----------------------------|----------------------|
| Vercel | Yes | $20/month | 100GB | 6,000 min/month |
| Netlify | Yes | $19/month | 100GB | 300 min/month |
| Cloudflare Pages | Yes | $20/month | Unlimited | 500 builds/month |
| Render | Yes | $7/month | 100GB | 500 min/month |

## Implementation Considerations

### Backend API Communication

Regardless of which frontend hosting solution you choose, you'll need to:

1. **Configure CORS on the Fly.io backend**:
   ```rust
   // Example CORS configuration in Axum
   let cors = CorsLayer::new()
       .allow_origin("https://your-frontend-domain.com".parse::<HeaderValue>().unwrap())
       .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
       .allow_credentials(true)
       .allow_headers([AUTHORIZATION, CONTENT_TYPE]);
   ```

2. **Set API base URL as an environment variable** in your frontend deployment to make it environment-aware:
   ```javascript
   // Example in a React app
   const API_BASE = process.env.REACT_APP_API_URL || 'http://localhost:8080';
   ```

3. **Implement proper error handling** for cross-origin requests

### CI/CD Workflow

For optimal developer experience:

1. Configure GitHub Actions to:
   - Run tests before deployment
   - Lint code for quality assurance
   - Deploy automatically to staging/preview environment

2. Use branch-based preview deployments:
   - Vercel, Netlify, and Cloudflare Pages all support automatic preview deployments for each pull request
   - This allows testing changes in isolation before merging to main/production

### Environment Management

Set up multiple environments:

1. **Development**: Local development environment
2. **Preview/Staging**: Automatically deployed from development branches
3. **Production**: Deployed from main branch

Each environment should have appropriate environment variables configured.

## Conclusion

For the AutoSWE frontend, we recommend **Vercel** as the primary deployment platform due to its excellent developer experience, performance, and seamless integration capabilities with our Fly.io backend.

This combination provides:
- Global distribution for both frontend and backend
- Modern CI/CD workflows with preview deployments
- Strong performance and reliability
- Cost-effective scaling (free to start, reasonable paid tiers)
- Separate scaling for frontend and backend (crucial for optimizing costs)

The deployment can be completed in less than an hour, providing a fast, globally distributed frontend that connects securely to your Fly.io backend.
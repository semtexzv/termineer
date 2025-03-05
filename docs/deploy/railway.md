# Railway Deployment Guide for AutoSWE Server

Railway is a modern platform-as-a-service (PaaS) designed to simplify the deployment process with an emphasis on developer experience. It offers a streamlined workflow from development to production with usage-based pricing.

## Overview

- **Platform Type**: Developer-friendly PaaS
- **Deployment Method**: Git-based or Docker container
- **Database**: PostgreSQL with one-click provisioning
- **Scaling**: Basic horizontal and vertical scaling
- **Pricing Model**: Usage-based with no free tier

## Pricing (as of 2024)

Railway uses a usage-based pricing model that charges based on actual resource consumption:

### Compute Resources
- **Base price**: $5/month minimum for active projects
- **Compute**: $0.000135/mCPU/second ($0.486/CPU/hour)
- **Memory**: $0.000009/MB/second ($0.032/GB/hour)
- **Disk**: $0.000005/MB/second ($0.018/GB/hour)

### PostgreSQL Database
- **Compute**: Same as above
- **Memory**: Same as above
- **Storage**: Same as above

### Example Cost Calculations

For a basic AutoSWE setup with:
- Web service: 0.5 CPU, 512MB RAM
- PostgreSQL: 0.5 CPU, 1GB RAM, 10GB storage

**Estimated monthly cost**:
- Web service: ~$20-30/month
- Database: ~$25-35/month
- **Total**: ~$45-65/month (varies based on usage)

### Additional Costs
- **Bandwidth**: Included (fair usage applies)
- **Custom Domains**: Included
- **SSL Certificates**: Included

## Pros and Cons

### Pros
- Exceptionally simple deployment process
- Intuitive user interface
- Excellent developer experience
- Quick setup of PostgreSQL databases
- Simple environment variable management
- Automatic deployments from Git
- Built-in monitoring and logging
- Shareable preview environments
- Team collaboration features

### Cons
- No free tier (minimum $5/month)
- Usage-based pricing can be unpredictable
- Limited granular control compared to AWS
- Fewer scaling options than Fly.io or AWS
- Limited global region options
- Not as cost-effective for high-resource applications
- Less mature than some alternatives

## Setup Instructions

1. **Create a Railway Account**:
   - Sign up at [railway.app](https://railway.app/)
   - Connect your GitHub account

2. **Create a New Project**:
   - Click "New Project"
   - Select "Deploy from GitHub repo"
   - Choose your AutoSWE server repository

3. **Configure the Rust Service**:
   - The service will automatically detect it's a Rust project
   - Configure build settings:
     - Build command: `cargo build --release`
     - Start command: `./target/release/autoswe-server`

4. **Add a PostgreSQL Database**:
   - Click "New" > "Database" > "PostgreSQL"
   - The database will be provisioned automatically

5. **Configure Environment Variables**:
   - Go to your service's "Variables" tab
   - Add the required environment variables:
     - `DATABASE_URL`: Will be auto-populated from the PostgreSQL service
     - `JWT_SECRET`: Your secure JWT secret
     - `GOOGLE_CLIENT_ID`: Your Google client ID
     - `GOOGLE_CLIENT_SECRET`: Your Google client secret
     - `OAUTH_REDIRECT_URL`: `https://your-service-name.up.railway.app/auth/google/callback`
     - `PORT`: 8080

6. **Deploy the Application**:
   - Railway will automatically deploy your application
   - You can view logs and metrics in real-time

7. **Configure Custom Domain**:
   - Go to your service's "Settings" > "Custom Domain"
   - Enter your domain name
   - Follow the DNS configuration instructions
   - Railway will automatically provision an SSL certificate

## Resource Configuration

Railway allows you to configure the resources allocated to your service:

1. **Scaling Configuration**:
   - Go to your service's "Settings"
   - Under "Resources", adjust CPU and memory allocation
   - For AutoSWE server, recommended settings are:
     - 0.5-1.0 CPU
     - 512MB-1GB Memory

2. **Database Configuration**:
   - Go to your PostgreSQL service's "Settings"
   - Under "Resources", adjust CPU, memory, and storage
   - For AutoSWE database, recommended settings are:
     - 0.5-1.0 CPU
     - 1-2GB Memory
     - 10GB Storage

## Recommendation

Railway is recommended for AutoSWE deployment when:

1. You prioritize developer experience and simplicity over cost
2. You want the quickest possible deployment process
3. Your application has moderate resource requirements
4. You don't need advanced scaling features
5. You prefer usage-based billing over fixed pricing

Railway offers a great balance of simplicity and functionality, making it ideal for developer-focused teams who want to get up and running quickly without managing infrastructure. It's particularly well-suited for startups and small teams where development velocity is a priority.

The usage-based pricing model means you pay for exactly what you use, which can be advantageous for applications with variable workloads. However, it also means costs can fluctuate from month to month, making budget planning slightly more challenging.

For the AutoSWE server, Railway provides a straightforward deployment path with all the necessary components (Rust build support, PostgreSQL database, custom domains) readily available. The platform's simplicity allows you to focus on application development rather than infrastructure management.
# Digital Ocean App Platform Deployment Guide for AutoSWE Server

Digital Ocean App Platform offers a managed container deployment solution that simplifies the process of deploying web applications. It's a good middle ground between the simplicity of Render and the power of AWS.

## Overview

- **Platform Type**: Managed container platform
- **Deployment Method**: Git-based or Docker container
- **Database**: Managed PostgreSQL
- **Scaling**: Horizontal and vertical scaling options
- **Pricing Model**: Tiered pricing with free tier for static sites

## Pricing (as of 2024)

### Container Instances

#### Shared CPU
- **512 MiB RAM / 1 vCPU**: $5/month (50 GiB transfer)
- **1 GiB RAM / 1 vCPU**: $10/month (100 GiB transfer)
- **1 GiB RAM / 1 vCPU**: $12/month (150 GiB transfer)
- **2 GiB RAM / 1 vCPU**: $25/month (200 GiB transfer)
- **4 GiB RAM / 2 vCPUs**: $50/month (250 GiB transfer)

#### Dedicated CPU (with autoscaling support)
- **512 MiB RAM / 1 vCPU**: $29/month (100 GiB transfer)
- **1 GiB RAM / 1 vCPU**: $34/month (200 GiB transfer)
- **2 GiB RAM / 1 vCPU**: $39/month (300 GiB transfer)
- **4 GiB RAM / 1 vCPU**: $49/month (400 GiB transfer)
- **4 GiB RAM / 2 vCPUs**: $78/month (500 GiB transfer)
- **8 GiB RAM / 2 vCPUs**: $98/month (600 GiB transfer)
- **8 GiB RAM / 4 vCPUs**: $156/month (700 GiB transfer)
- **16 GiB RAM / 4 vCPUs**: $196/month (800 GiB transfer)
- **32 GiB RAM / 8 vCPUs**: $392/month (900 GiB transfer)

### PostgreSQL Database

Digital Ocean offers managed databases separate from App Platform:

- **Development Database (512 MiB)**: $7/month
- **Basic Tier (1GB)**: $15/month
- **Professional Tier (4GB)**: $60/month
- **Plus additional storage at $0.10/GB**

### Other Costs
- **Dedicated Egress IPs**: $25/month per app
- **Additional Outbound Transfer**: $0.02/GiB

## Pros and Cons

### Pros
- Easy deployment via GitHub, GitLab, or container registry
- Good scaling options with both horizontal and vertical scaling
- Built-in support for PostgreSQL databases
- Autoscaling for dedicated instances
- Global CDN for faster content delivery
- Zero-downtime deploys
- SSH access for debugging
- DDoS protection included

### Cons
- More expensive than Render or Fly.io for similar resources
- Limited free tier (only for static sites)
- Development database has limited capabilities
- Fewer global regions than Fly.io
- Not as feature-rich as AWS

## Setup Instructions

1. **Create a Digital Ocean Account**:
   - Sign up at [digitalocean.com](https://www.digitalocean.com/)

2. **Create a New App**:
   - Go to the App Platform section
   - Click "Create App"
   - Choose your source repository (GitHub, GitLab) or container registry

3. **Configure the App**:
   - Select the repository containing your AutoSWE server code
   - Set the branch to deploy from
   - Configure the build and run commands:
     - Build command: `cargo build --release`
     - Run command: `./target/release/autoswe-server`

4. **Select Resources**:
   - Choose the appropriate container size (recommended: at least 1GB RAM)
   - Configure scaling options if needed

5. **Add a Database**:
   - Click "Add a Database"
   - Choose PostgreSQL
   - Select the appropriate size (at least Basic-1GB for production)

6. **Configure Environment Variables**:
   - Add all required environment variables:
     - `DATABASE_URL`: Will be auto-populated when you add a database
     - `JWT_SECRET`: Your secure JWT secret
     - `GOOGLE_CLIENT_ID`: Your Google client ID
     - `GOOGLE_CLIENT_SECRET`: Your Google client secret
     - `OAUTH_REDIRECT_URL`: `https://your-app.ondigitalocean.app/auth/google/callback`

7. **Configure Additional Settings**:
   - Set up Health Checks
   - Configure custom domains if needed
   - Enable automatic deploys

8. **Launch the App**:
   - Review all settings
   - Click "Launch App"

## Custom Domain Configuration

1. Go to your app's settings
2. Click "Domains"
3. Click "Add Domain"
4. Enter your domain name
5. Follow the DNS configuration instructions
6. Wait for DNS propagation and SSL certificate issuance

## Recommended Configuration for AutoSWE Server

For the AutoSWE server, we recommend the following configuration:

- **Service Type**: Web Service
- **Container Size**: Dedicated 1 vCPU / 2GB RAM ($39/month)
- **Database**: Basic-1GB ($15/month)
- **Total Estimated Cost**: $54/month

This configuration provides sufficient resources to handle moderate traffic while maintaining good performance and reliability.

## Monitoring and Scaling

Digital Ocean App Platform provides built-in monitoring tools and autoscaling capabilities (for dedicated instances):

1. **Vertical Scaling**:
   - Increase resources by upgrading to a larger container size
   - No downtime required

2. **Horizontal Scaling**:
   - Increase the number of container instances
   - Load balancing is handled automatically

3. **Monitoring**:
   - CPU and memory usage metrics
   - HTTP request metrics
   - Custom alerts can be configured

## Recommendation

Digital Ocean App Platform is recommended for AutoSWE deployment when:

1. You need a balance of simplicity and control
2. You're comfortable with slightly higher costs for a more managed experience
3. You need good scaling options but don't want the complexity of AWS
4. You need built-in PostgreSQL database integration
5. You want a platform with excellent documentation and community support

Digital Ocean App Platform provides a good middle ground between the simplicity of Render and the power of AWS, making it a solid choice for small to medium-sized teams who want to focus on development rather than infrastructure management.
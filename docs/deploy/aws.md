# AWS Deployment Guide for AutoSWE Server

Amazon Web Services (AWS) provides a comprehensive and highly configurable platform for deploying applications. While more complex than other options, AWS offers unmatched scalability, reliability, and a wide range of services that can be tailored to your specific needs.

## Overview

- **Platform Type**: Comprehensive cloud platform
- **Deployment Options**: Multiple (EC2, ECS, Elastic Beanstalk, App Runner)
- **Database**: Amazon RDS for PostgreSQL
- **Scaling**: Highly configurable auto-scaling
- **Pricing Model**: Pay-as-you-go with complex pricing structure

## Deployment Options

AWS offers several ways to deploy the AutoSWE server:

1. **AWS App Runner** - Simplest option, fully managed
2. **Elastic Beanstalk** - Simple deployment with more customization
3. **ECS/Fargate** - Container-based deployment with good scalability
4. **EC2 Instances** - Maximum control and customization

## Pricing (as of 2024)

AWS pricing is complex and depends on many factors. Here are approximate costs for a typical deployment:

### AWS App Runner
- **Compute**: Starting at $0.064/vCPU-hour, $0.0018/GB-hour
- **Estimated monthly cost**: $25-$75/month for basic setup

### EC2 (t4g.small - 2 vCPU, 2GB RAM)
- **Compute**: ~$14/month (On-Demand)
- **Data Transfer**: $0.09/GB after first 100GB
- **Elastic IP**: $3.60/month if unused

### RDS for PostgreSQL (db.t4g.small - 2 vCPU, 2GB RAM)
- **Database**: ~$30/month
- **Storage**: $0.115/GB-month
- **Backup Storage**: $0.095/GB-month

### Additional Costs
- **Load Balancer**: $16.20/month
- **NAT Gateway**: $32.40/month + $0.045/GB data processed
- **Route 53**: $0.50/hosted zone/month + $0.40/million queries

### Estimated Total
- **Basic Setup (App Runner + RDS)**: $55-$105/month
- **Production Setup (EC2 + RDS + ELB)**: $80-$150/month

## Pros and Cons

### Pros
- Extremely scalable and reliable
- Wide range of services and integrations
- Granular control over infrastructure
- Superior security options
- Global infrastructure with multiple regions
- Extensive monitoring and logging capabilities
- Mature CI/CD integration
- Managed SSL certificates with ACM

### Cons
- Steeper learning curve
- More complex setup process
- Higher operational overhead
- More complex pricing structure
- Potential for unexpected costs
- Requires more DevOps knowledge

## Setup Instructions for AWS App Runner (Simplest Option)

1. **Create an AWS Account**:
   - Sign up at [aws.amazon.com](https://aws.amazon.com/)

2. **Create an RDS PostgreSQL Database**:
   - Navigate to RDS in the AWS console
   - Click "Create database"
   - Select PostgreSQL
   - Choose "Standard Create"
   - Select "Dev/Test" or "Production"
   - Choose db.t4g.small for Dev/Test or db.t4g.medium for Production
   - Set up username and password
   - Configure network settings (VPC, security groups)
   - Create database

3. **Set Up App Runner**:
   - Navigate to AWS App Runner in the console
   - Click "Create service"
   - Choose "Source code repository" and connect your GitHub/GitLab repo
   - Configure the build:
     - Runtime: Custom
     - Build command: `cargo build --release`
     - Start command: `./target/release/autoswe-server`
   - Configure service settings:
     - CPU: 1 vCPU
     - Memory: 2 GB
   - Configure environment variables:
     - `DATABASE_URL`: Your RDS connection string
     - `JWT_SECRET`: Your secure JWT secret
     - `GOOGLE_CLIENT_ID`: Your Google client ID
     - `GOOGLE_CLIENT_SECRET`: Your Google client secret
     - `OAUTH_REDIRECT_URL`: Auto-generated App Runner URL path

4. **Set Up Custom Domain**:
   - In App Runner service, go to "Custom domains"
   - Add your domain name
   - Configure DNS with provided certificate validation records
   - Wait for certificate validation and DNS propagation

## Alternative: EC2-based Deployment

For more control, an EC2-based deployment using Docker containers is recommended:

1. **Create VPC and Networking**:
   - Create a VPC with public and private subnets
   - Set up Internet Gateway, NAT Gateway, and route tables

2. **Create Security Groups**:
   - Web tier security group (allow HTTP/HTTPS)
   - Database security group (allow PostgreSQL from web tier)

3. **Launch EC2 Instances**:
   - Use Amazon Linux 2
   - t4g.small or larger instance type
   - Install Docker and configure to run on startup

4. **Deploy Application**:
   - Create a deployment script or use AWS CodeDeploy
   - Configure Docker to run your application container
   - Set up environment variables

5. **Set Up Load Balancer**:
   - Create an Application Load Balancer
   - Configure Target Groups pointing to your EC2 instances
   - Set up SSL certificates via ACM

6. **Configure Auto Scaling**:
   - Create Launch Templates
   - Set up Auto Scaling Group
   - Configure scaling policies based on CPU/memory usage

## Maintenance and Operations

AWS requires more operational oversight than other platforms:

1. **Monitoring**:
   - Set up CloudWatch Alarms for CPU, memory, disk usage
   - Configure CloudWatch Logs for application logging
   - Create dashboards for key metrics

2. **Backup Strategy**:
   - Enable automated RDS backups
   - Configure backup retention periods
   - Test restoration procedures

3. **Security Updates**:
   - Regularly patch EC2 instances
   - Keep Docker images updated
   - Rotate access credentials

4. **Cost Optimization**:
   - Use Reserved Instances for predictable workloads
   - Configure Auto Scaling to scale down during low-traffic periods
   - Set up AWS Budgets and Cost Alarms

## Recommendation

AWS is recommended for AutoSWE deployment when:

1. You need maximum scalability and reliability
2. You have specific compliance or security requirements
3. You have or plan to develop DevOps expertise
4. You anticipate complex infrastructure needs
5. You want to leverage other AWS services (S3, CloudFront, etc.)

For teams starting out or with limited DevOps resources, begin with AWS App Runner for simplicity. As your needs grow, you can transition to more customized EC2 or container-based deployments for greater control and cost optimization.

AWS offers the most powerful and flexible deployment option, but requires more investment in learning and operations than other platforms.
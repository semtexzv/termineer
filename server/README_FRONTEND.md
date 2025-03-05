# AutoSWE Frontend with HTMX and Axum

This document describes the frontend implementation for the AutoSWE server using HTMX and Askama templates.

## Architecture

The frontend uses a server-side rendering approach with the following technologies:

- **HTMX**: For interactive UI without requiring a JavaScript framework
- **Askama**: For server-side templating in Rust
- **Tailwind CSS**: For styling (via CDN for simplicity)
- **Axum**: Rust web framework serving both the API and frontend

This approach offers several advantages:

- No complex build process for the frontend
- Server-side rendering for fast initial page loads
- Progressive enhancement with HTMX
- Type-safe templates compiled with Rust

## Directory Structure

```
server/
├── src/
│   ├── main.rs - Main application entry point with route handlers
│   ├── templates.rs - Template definitions and rendering
│   ├── middleware.rs - User authentication middleware
│   └── ... (other server modules)
├── templates/
│   ├── base.html - Base template with common layout
│   ├── index.html - Home page template
│   ├── checkout.html - Checkout form template
│   └── components/
│       ├── auth_button.html - Authentication button component
│       └── pricing.html - Pricing cards component
├── static/
│   └── js/
│       └── htmx.min.js - HTMX library
└── Dockerfile.web - Docker configuration for deployment
```

## Local Development

1. Make sure you have Rust installed on your system
2. Navigate to the server directory:
   ```
   cd server
   ```
3. Start the server:
   ```
   cargo run
   ```
4. Visit http://localhost:3000 in your browser

## Deployment to Fly.io

The project includes configuration for deploying to Fly.io:

1. Install the Fly CLI:
   ```
   curl -L https://fly.io/install.sh | sh
   ```

2. Log in to Fly:
   ```
   fly auth login
   ```

3. Launch the application:
   ```
   fly launch --dockerfile Dockerfile.web
   ```

4. Set required secrets:
   ```
   fly secrets set DATABASE_URL="postgres://username:password@hostname:5432/autoswe"
   fly secrets set GOOGLE_CLIENT_ID="your-google-client-id"
   fly secrets set GOOGLE_CLIENT_SECRET="your-google-client-secret"
   fly secrets set STRIPE_SECRET_KEY="your-stripe-secret-key"
   ```

5. Deploy the application:
   ```
   fly deploy
   ```

## How It Works

### Templates and Components

The Askama templates are compiled at build time and provide type-safe rendering in Rust. The templates use a component-based approach:

- `base.html`: Contains the page structure, header, and footer
- `index.html`: Extends base and includes pricing component
- Components are included using Askama's `{% include %}` directive

### HTMX Integration

HTMX enables dynamic behavior without complex JavaScript:

1. **Authentication**: The auth button is loaded via HTMX when the page loads
   ```html
   <div id="auth-container" hx-get="/auth/status" hx-trigger="load" hx-swap="innerHTML">
   ```

2. **Checkout Flow**: Clicking a subscription button triggers a POST request
   ```html
   <button
       hx-post="/payment/checkout"
       hx-vals='{"plan": "plus"}'
       hx-target="#checkout-container"
       class="...">
       Subscribe to Plus
   </button>
   ```

### User Authentication

User authentication is handled via middleware that:

1. Checks if a user is logged in via session
2. Attaches the user object to the request if authenticated
3. Provides user information to templates for personalized content

## Customization

### Changing Styles

The frontend uses Tailwind CSS via CDN for simplicity. To modify styles:

- Edit the class attributes in the HTML templates
- For custom styles beyond Tailwind, add them to the `<style>` section in `base.html`

### Adding New Pages

To add a new page:

1. Create a new template in the `templates` directory
2. Define a template struct in `templates.rs`
3. Add a new route handler in `main.rs`
4. Register the route in the app router

Example:

```rust
// In templates.rs
#[derive(Template)]
#[template(path = "about.html")]
pub struct AboutTemplate;

// In main.rs
async fn about_handler() -> impl IntoResponse {
    AboutTemplate.into()
}

// In router setup
.route("/about", get(about_handler))
```

### Custom Components

To create a reusable component:

1. Create a new file in `templates/components/`
2. Use it in other templates with `{% include "components/your_component.html" %}`
3. Pass variables from the parent template to the component
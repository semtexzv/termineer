/**
 * Authentication handler for Termineer
 * Manages authentication state and UI updates
 */

// Global auth state
window.termineer = window.termineer || {};
window.termineer.auth = {
  authenticated: false,
  user: null,
  isLoading: true
};

document.addEventListener('DOMContentLoaded', function() {
  // Initialize UI elements
  const loginButton = document.getElementById('auth-button');
  const userInfoDisplay = document.getElementById('user-info-display');
  const buttonContent = loginButton?.querySelector('.button-content');
  const loadingIndicator = loginButton?.querySelector('.loading-indicator');
  
  // Ensure elements exist
  if (!loginButton || !userInfoDisplay || !buttonContent || !loadingIndicator) {
    console.error('Auth UI elements not found');
    return;
  }
  
  // Set initial loading state
  buttonContent.classList.add('hidden');
  loadingIndicator.classList.remove('hidden');
  
  // Fetch user authentication status
  fetchUserStatus()
    .then(userData => {
      // Update state
      window.termineer.auth.authenticated = !!userData;
      window.termineer.auth.user = userData;
      window.termineer.auth.isLoading = false;
      
      // Update UI
      updateAuthUI();
    })
    .catch(error => {
      // Handle error - show login button
      console.error('Error fetching auth status:', error);
      window.termineer.auth.isLoading = false;
      window.termineer.auth.authenticated = false;
      updateAuthUI();
    });
});

/**
 * Update authentication UI based on current state
 */
function updateAuthUI() {
  const loginButton = document.getElementById('auth-button');
  const userInfoDisplay = document.getElementById('user-info-display');
  const buttonContent = loginButton?.querySelector('.button-content');
  const loadingIndicator = loginButton?.querySelector('.loading-indicator');
  
  if (!loginButton || !userInfoDisplay || !buttonContent || !loadingIndicator) {
    return;
  }
  
  // Loading state finished, hide spinner
  loadingIndicator.classList.add('hidden');
  
  if (window.termineer.auth.authenticated && window.termineer.auth.user) {
    // User is authenticated - show user info, hide login button
    loginButton.classList.add('hidden');
    userInfoDisplay.classList.remove('hidden');
    
    // Update user information
    const user = window.termineer.auth.user;
    const nameElement = document.getElementById('user-name');
    const subElement = document.getElementById('user-subscription');
    
    if (nameElement) {
      nameElement.textContent = user.name || user.email;
    }
    
    if (subElement) {
      subElement.textContent = user.subscription || 'Free';
    }
  } else {
    // User is not authenticated - show login button, hide user info
    loginButton.classList.remove('hidden');
    buttonContent.classList.remove('hidden');
    userInfoDisplay.classList.add('hidden');
  }
}

/**
 * Fetch current user authentication status from API
 * @returns {Promise<Object|null>} User data or null if not authenticated
 */
async function fetchUserStatus() {
  try {
    const response = await fetch('/api/auth/status', {
      method: 'GET',
      credentials: 'same-origin',
      headers: {
        'Accept': 'application/json'
      }
    });
    
    if (!response.ok) {
      throw new Error(`Status error: ${response.status}`);
    }
    
    const data = await response.json();
    return data.user;
  } catch (error) {
    console.error('Failed to fetch auth status:', error);
    return null;
  }
}
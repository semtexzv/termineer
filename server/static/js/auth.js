/**
 * Authentication utilities for Termineer
 */

document.addEventListener('DOMContentLoaded', function() {
    const authContainer = document.getElementById('auth-container');
    
    // If we're on a page with the auth container
    if (authContainer) {
        // Fetch authentication status on page load
        fetchAuthStatus();
    }
});

/**
 * Fetch current authentication status from the API
 */
async function fetchAuthStatus() {
    try {
        const response = await fetch('/api/auth/status');
        if (!response.ok) {
            throw new Error(`HTTP error! Status: ${response.status}`);
        }
        
        const data = await response.json();
        
        // Update UI based on authentication status
        if (data.authenticated && data.user) {
            // User is authenticated
            updateUIForAuthenticatedUser(data.user);
        } else {
            // User is not authenticated
            updateUIForUnauthenticatedUser();
        }
    } catch (error) {
        console.error('Error fetching authentication status:', error);
    }
}

/**
 * Update UI when user is authenticated
 */
function updateUIForAuthenticatedUser(user) {
    const authContainer = document.getElementById('auth-container');
    
    // Only update if not already showing authenticated state
    if (!authContainer.querySelector('#user-menu-button')) {
        const userInitial = user.name ? user.name.charAt(0).toUpperCase() : user.email.charAt(0).toUpperCase();
        
        const html = `
            <button id="user-menu-button" class="flex items-center text-gray-700 hover:text-primary-600 transition-colors">
                ${user.picture 
                    ? `<img src="${user.picture}" alt="${user.name || user.email}" class="w-8 h-8 rounded-full mr-2">` 
                    : `<div class="w-8 h-8 rounded-full bg-primary-600 text-white flex items-center justify-center mr-2">${userInitial}</div>`
                }
                <span class="font-medium">${user.name || user.email}</span>
                <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5 ml-1" viewBox="0 0 20 20" fill="currentColor">
                    <path fill-rule="evenodd" d="M5.293 7.293a1 1 0 011.414 0L10 10.586l3.293-3.293a1 1 0 111.414 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 010-1.414z" clip-rule="evenodd" />
                </svg>
            </button>
            <div id="user-dropdown" class="absolute right-0 mt-2 w-48 bg-white rounded-md shadow-lg py-1 hidden">
                <div class="px-4 py-2 border-b border-gray-100">
                    <p class="text-sm font-medium text-gray-900 truncate">${user.name || "User"}</p>
                    <p class="text-sm text-gray-500 truncate">${user.email}</p>
                </div>
                <a href="/auth/logout" class="block px-4 py-2 text-sm text-gray-700 hover:bg-gray-100">Sign out</a>
            </div>
        `;
        
        authContainer.innerHTML = html;
        
        // Add event listeners for dropdown menu
        setupUserDropdownEvents();
    }
}

/**
 * Update UI when user is not authenticated
 */
function updateUIForUnauthenticatedUser() {
    const authContainer = document.getElementById('auth-container');
    
    // Only update if not already showing login button
    if (!authContainer.querySelector('a[href="/auth/google/login"]')) {
        const html = `
            <a href="/auth/google/login" class="flex items-center bg-white border border-gray-300 rounded-md px-4 py-2 text-gray-700 font-medium hover:bg-gray-50 transition-colors">
                <svg class="w-5 h-5 mr-2" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
                    <path fill="#4285F4" d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z" />
                    <path fill="#34A853" d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z" />
                    <path fill="#FBBC05" d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z" />
                    <path fill="#EA4335" d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z" />
                    <path fill="none" d="M1 1h22v22H1z" />
                </svg>
                Sign in with Google
            </a>
        `;
        
        authContainer.innerHTML = html;
    }
}

/**
 * Set up event listeners for user dropdown
 */
function setupUserDropdownEvents() {
    const userMenuButton = document.getElementById('user-menu-button');
    const userDropdown = document.getElementById('user-dropdown');
    
    if (userMenuButton && userDropdown) {
        // Toggle dropdown when clicking the button
        userMenuButton.addEventListener('click', function(event) {
            event.stopPropagation();
            userDropdown.classList.toggle('hidden');
        });
        
        // Close dropdown when clicking outside
        document.addEventListener('click', function() {
            if (!userDropdown.classList.contains('hidden')) {
                userDropdown.classList.add('hidden');
            }
        });
        
        // Prevent dropdown from closing when clicking inside it
        userDropdown.addEventListener('click', function(event) {
            event.stopPropagation();
        });
    }
}
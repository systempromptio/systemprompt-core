#[allow(dead_code)]
pub const SUCCESS_HTML: &str = r##"<!DOCTYPE html>
<html>
<head>
    <title>Purchase Successful - SystemPrompt</title>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            display: flex;
            justify-content: center;
            align-items: center;
            min-height: 100vh;
            margin: 0;
            background: #0a0a0a;
            color: white;
        }
        .container {
            text-align: center;
            padding: 48px;
            max-width: 500px;
        }
        .success-icon {
            width: 64px;
            height: 64px;
            border-radius: 50%;
            background: #22c55e;
            display: flex;
            align-items: center;
            justify-content: center;
            margin: 0 auto 24px;
            font-size: 32px;
        }
        .spinner {
            width: 24px;
            height: 24px;
            border: 3px solid #27272a;
            border-top-color: #FF9A2F;
            border-radius: 50%;
            animation: spin 1s linear infinite;
            margin: 0 auto 16px;
        }
        @keyframes spin {
            to { transform: rotate(360deg); }
        }
        h1 {
            margin: 0 0 12px;
            font-size: 1.5em;
            font-weight: 600;
        }
        p {
            margin: 0 0 8px;
            color: #a1a1aa;
            font-size: 0.95em;
        }
        .status-container {
            margin-top: 24px;
            padding: 24px;
            background: #18181b;
            border-radius: 12px;
        }
        .status-message {
            color: #FF9A2F;
            font-weight: 500;
        }
        .ready-container {
            margin-top: 16px;
        }
        .url-link {
            color: #FF9A2F;
            text-decoration: none;
            word-break: break-all;
        }
        .url-link:hover {
            text-decoration: underline;
        }
        .done-message {
            margin-top: 16px;
            color: #71717a;
            font-size: 0.85em;
        }
    </style>
</head>
<body>
    <div class="container">
        <div class="success-icon">✓</div>
        <h1>Purchase Successful!</h1>
        <p>Your tenant is being provisioned...</p>

        <div class="status-container" id="status-container">
            <div class="spinner" id="spinner"></div>
            <p class="status-message" id="status-message">Initializing...</p>
        </div>
    </div>

    <script>
        const tenantId = '{{TENANT_ID}}';
        const pollInterval = 2000;

        async function checkStatus() {
            try {
                const response = await fetch(`/status/${tenantId}`);
                const data = await response.json();

                const statusMessage = document.getElementById('status-message');
                const spinner = document.getElementById('spinner');
                const statusContainer = document.getElementById('status-container');

                if (data.status === 'ready' || data.status === 'deployed') {
                    spinner.style.display = 'none';
                    statusContainer.innerHTML = `
                        <div class="ready-container">
                            <p style="color: #22c55e; font-weight: 600; margin-bottom: 12px;">Tenant Ready!</p>
                            ${data.app_url ? `<p>URL: <a href="${data.app_url}" class="url-link" target="_blank">${data.app_url}</a></p>` : ''}
                            <p class="done-message">You can close this window and return to the terminal.</p>
                        </div>
                    `;
                } else if (data.status === 'error' || data.status === 'failed') {
                    spinner.style.display = 'none';
                    statusMessage.style.color = '#ef4444';
                    statusMessage.textContent = data.message || 'Provisioning failed';
                } else {
                    statusMessage.textContent = data.message || 'Provisioning...';
                    setTimeout(checkStatus, pollInterval);
                }
            } catch (e) {
                console.error('Status check failed:', e);
                setTimeout(checkStatus, pollInterval);
            }
        }

        // Start polling
        checkStatus();
    </script>
</body>
</html>"##;

#[allow(dead_code)]
pub const ERROR_HTML: &str = r##"<!DOCTYPE html>
<html>
<head>
    <title>Checkout Failed - SystemPrompt</title>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            display: flex;
            justify-content: center;
            align-items: center;
            min-height: 100vh;
            margin: 0;
            background: #0a0a0a;
            color: white;
        }
        .container {
            text-align: center;
            padding: 48px;
            max-width: 400px;
        }
        .error-icon {
            width: 64px;
            height: 64px;
            border-radius: 50%;
            background: #ef4444;
            display: flex;
            align-items: center;
            justify-content: center;
            margin: 0 auto 24px;
            font-size: 32px;
        }
        h1 {
            margin: 0 0 12px;
            font-size: 1.5em;
            font-weight: 600;
        }
        p {
            margin: 0;
            color: #a1a1aa;
            font-size: 0.95em;
        }
    </style>
</head>
<body>
    <div class="container">
        <div class="error-icon">✗</div>
        <h1>Checkout Failed</h1>
        <p>Please try again or contact support.</p>
    </div>
</body>
</html>"##;

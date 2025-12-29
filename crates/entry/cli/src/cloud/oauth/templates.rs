pub const SUCCESS_HTML: &str = r##"<!DOCTYPE html>
<html>
<head>
    <title>Authentication Successful - SystemPrompt</title>
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
        .success-icon {
            width: 64px;
            height: 64px;
            border-radius: 50%;
            background: #FF9A2F;
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
        <div class="success-icon">✓</div>
        <h1>Authentication Successful</h1>
        <p>You can close this window and return to the terminal.</p>
    </div>
</body>
</html>"##;

pub const ERROR_HTML: &str = r##"<!DOCTYPE html>
<html>
<head>
    <title>Authentication Failed - SystemPrompt</title>
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
        <h1>Authentication Failed</h1>
        <p>Please try again.</p>
    </div>
</body>
</html>"##;

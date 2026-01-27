document.getElementById('copy-btn').addEventListener('click', async () => {
    const content = document.getElementById('text-content').innerText;
    try {
        await navigator.clipboard.writeText(content);
        const btn = document.getElementById('copy-btn');
        btn.innerHTML = '<span class="copy-icon">\u2713</span> Copied!';
        setTimeout(() => {
            btn.innerHTML = '<span class="copy-icon">\uD83D\uDCCB</span> Copy';
        }, 2000);
    } catch (err) {
        console.error('Failed to copy:', err);
    }
});

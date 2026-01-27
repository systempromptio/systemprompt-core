let chartInstance = null;

function initChart() {
    const ctx = document.getElementById('chart');
    if (!ctx) return;

    const config = window.CHART_CONFIG;

    chartInstance = new Chart(ctx, config);
}

if (typeof Chart !== 'undefined') {
    document.addEventListener('DOMContentLoaded', initChart);
} else {
    const script = document.createElement('script');
    script.src = window.CHART_JS_CDN;
    script.onload = initChart;
    document.head.appendChild(script);
}

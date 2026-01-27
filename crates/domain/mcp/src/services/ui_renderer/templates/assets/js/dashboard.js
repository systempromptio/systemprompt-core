const DashboardApp = {
    charts: [],
    chartConfigs: window.DASHBOARD_CHART_CONFIGS,

    init() {
        this.initCharts();
        this.initTabs();
    },

    initCharts() {
        if (typeof Chart === 'undefined') {
            const script = document.createElement('script');
            script.src = window.CHART_JS_CDN;
            script.onload = () => this.createCharts();
            document.head.appendChild(script);
        } else {
            this.createCharts();
        }
    },

    createCharts() {
        this.chartConfigs.forEach(config => {
            const canvas = document.getElementById(config.id);
            if (canvas) {
                this.charts.push(new Chart(canvas, {
                    type: config.type,
                    data: config.data,
                    options: config.options
                }));
            }
        });
    },

    initTabs() {
        document.querySelectorAll('.tab-btn').forEach(btn => {
            btn.addEventListener('click', (e) => {
                const target = e.target.dataset.target;

                document.querySelectorAll('.tab-btn').forEach(b => b.classList.remove('active'));
                e.target.classList.add('active');

                document.querySelectorAll('.dashboard-section').forEach(s => {
                    s.style.display = s.id === target ? 'block' : 'none';
                });
            });
        });

        const sections = document.querySelectorAll('.layout-tabs .dashboard-section');
        sections.forEach((s, i) => {
            if (i > 0) s.style.display = 'none';
        });
    }
};

document.addEventListener('DOMContentLoaded', () => DashboardApp.init());

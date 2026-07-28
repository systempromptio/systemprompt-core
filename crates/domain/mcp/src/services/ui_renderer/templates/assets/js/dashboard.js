// Charts are server-rendered SVG, so the only behaviour a dashboard needs is
// tab switching.
const DashboardApp = {
    init() {
        const buttons = document.querySelectorAll('.tab-btn');
        buttons.forEach(btn => {
            btn.addEventListener('click', () => this.select(btn, buttons));
        });

        document.querySelectorAll('.layout-tabs').forEach(layout => {
            layout.querySelectorAll('.dashboard-section').forEach((section, i) => {
                section.hidden = i > 0;
            });
            layout.classList.add('tabs-ready');
        });
    },

    select(active, buttons) {
        buttons.forEach(btn => btn.classList.toggle('active', btn === active));
        document.querySelectorAll('.dashboard-section').forEach(section => {
            section.hidden = section.id !== active.dataset.target;
        });
    }
};

document.addEventListener('DOMContentLoaded', () => DashboardApp.init());

// Charts are server-rendered SVG, so the only behaviour a dashboard needs is
// tab switching.
const DashboardApp = {
    init() {
        document.querySelectorAll('.layout-tabs').forEach(layout => this.setupTabs(layout));
        document.querySelectorAll('.section-table-sortable').forEach(t => this.setupSort(t));
        this.setupRefresh();
        this.setupDrillDown();
    },

    /* `hints.refreshable` and `refresh_interval_seconds` were parsed and
     * dropped entirely.
     *
     * This asks the model to re-run the tool rather than re-reading the
     * artifact resource: the resource is the *persisted* payload, so re-reading
     * it would faithfully redraw the same stale numbers. Only the tool can
     * produce fresh ones. */
    setupRefresh() {
        const btn = document.getElementById('dashboard-refresh');
        if (!btn) return;
        const status = document.getElementById('refresh-status');
        const title = (document.querySelector('.mcp-app-title')?.textContent ?? 'this dashboard').trim();

        const refresh = async () => {
            if (btn.disabled) return;
            btn.disabled = true;
            btn.setAttribute('aria-busy', 'true');
            if (status) status.textContent = 'Asking for fresh data…';
            try {
                await McpAppBridge.sendMessage(`Refresh ${title} with current data.`);
                if (status) status.textContent = 'Requested.';
            } catch (err) {
                console.error('Dashboard refresh failed:', err);
                if (status) status.textContent = 'Could not request a refresh.';
            } finally {
                btn.disabled = false;
                btn.removeAttribute('aria-busy');
            }
        };

        btn.addEventListener('click', refresh);

        /* An interval here puts a prompt in front of the model on a timer, so
         * the floor is deliberately high and it only runs while the tab is
         * visible. */
        const secs = Number(btn.dataset.refreshInterval);
        if (Number.isFinite(secs) && secs > 0) {
            setInterval(() => {
                if (document.visibilityState === 'visible') refresh();
            }, Math.max(30, secs) * 1000);
        }
    },

    /* `hints.drill_down_enabled` gates whether a table row asks the model
     * about itself. Without it the rows carry no affordance at all. */
    setupDrillDown() {
        const grid = document.querySelector('.dashboard[data-drill-down="true"]');
        if (!grid) return;
        for (const row of grid.querySelectorAll('.section-table tbody tr')) {
            row.tabIndex = 0;
            row.classList.add('is-drillable');
            const ask = () => {
                const cells = Array.from(row.children).map(c => c.textContent.trim());
                McpAppBridge.sendMessage(`Tell me more about this row: ${cells.join(' | ')}`)
                    .catch(err => console.error('Drill-down failed:', err));
            };
            row.addEventListener('click', ask);
            row.addEventListener('keydown', (e) => {
                if (e.key === 'Enter') { e.preventDefault(); ask(); }
            });
        }
    },

    /* `TableSectionData.sortable` was declared by the model and did nothing.
     * The server applies `default_sort`; this adds the click/keyboard sort on
     * top, matching the standalone table renderer's behaviour. */
    setupSort(table) {
        const tbody = table.querySelector('tbody');
        if (!tbody) return;
        const headers = Array.from(table.querySelectorAll('th.sortable'));

        headers.forEach((th, index) => {
            let direction = 'asc';
            const sort = () => {
                const rows = Array.from(tbody.querySelectorAll('tr'));
                rows.sort((a, b) => {
                    const av = (a.children[index]?.textContent ?? '').trim();
                    const bv = (b.children[index]?.textContent ?? '').trim();
                    const an = Number(av.replace(/,/g, ''));
                    const bn = Number(bv.replace(/,/g, ''));
                    const cmp = (av !== '' && bv !== '' && Number.isFinite(an) && Number.isFinite(bn))
                        ? an - bn
                        : av.localeCompare(bv, undefined, { numeric: true });
                    return direction === 'asc' ? cmp : -cmp;
                });
                rows.forEach(r => tbody.appendChild(r));
                headers.forEach(h => {
                    h.classList.remove('sort-asc', 'sort-desc');
                    h.removeAttribute('aria-sort');
                });
                th.classList.add(direction === 'asc' ? 'sort-asc' : 'sort-desc');
                th.setAttribute('aria-sort', direction === 'asc' ? 'ascending' : 'descending');
                direction = direction === 'asc' ? 'desc' : 'asc';
            };
            th.addEventListener('click', sort);
            th.addEventListener('keydown', (e) => {
                if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); sort(); }
            });
        });
    },

    /* Scoped to one layout. This used to query the whole document, so a
     * dashboard with more than one tabbed layout had every panel respond to
     * every tab. The tabpanel roles are applied here rather than server-side
     * because they are only true once the script has taken over — before that
     * the sections are a plain stacked list. */
    setupTabs(layout) {
        const buttons = Array.from(layout.querySelectorAll('.tab-btn'));
        const sections = Array.from(layout.querySelectorAll('.dashboard-section'));

        sections.forEach((section, i) => {
            section.setAttribute('role', 'tabpanel');
            section.setAttribute('aria-labelledby', `tab-${section.id}`);
            if (section.getAttribute('tabindex') === null) section.tabIndex = 0;
            section.hidden = i > 0;
        });

        buttons.forEach((btn, i) => {
            btn.addEventListener('click', () => this.select(i, buttons, sections));
            btn.addEventListener('keydown', (e) => {
                const last = buttons.length - 1;
                let next = null;
                if (e.key === 'ArrowRight' || e.key === 'ArrowDown') next = i === last ? 0 : i + 1;
                else if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') next = i === 0 ? last : i - 1;
                else if (e.key === 'Home') next = 0;
                else if (e.key === 'End') next = last;
                if (next !== null) {
                    e.preventDefault();
                    this.select(next, buttons, sections);
                    buttons[next].focus();
                }
            });
        });

        layout.classList.add('tabs-ready');
    },

    select(index, buttons, sections) {
        buttons.forEach((btn, i) => {
            const selected = i === index;
            btn.classList.toggle('active', selected);
            btn.setAttribute('aria-selected', String(selected));
            btn.tabIndex = selected ? 0 : -1;
        });
        const target = buttons[index].dataset.target;
        sections.forEach(section => {
            section.hidden = section.id !== target;
        });
    }
};

document.addEventListener('DOMContentLoaded', () => DashboardApp.init());

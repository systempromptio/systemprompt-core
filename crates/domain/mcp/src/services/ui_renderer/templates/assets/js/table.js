const TableApp = {
    columns: window.TABLE_COLUMNS,
    rows: window.TABLE_ROWS,
    sortableColumns: window.TABLE_SORTABLE,
    filterable: window.TABLE_FILTERABLE,
    pageSize: window.TABLE_PAGE_SIZE,
    currentPage: 1,
    sortColumn: null,
    sortDirection: 'asc',
    filterText: '',

    init() {
        this.renderHeader();
        this.renderBody();
        this.setupEventListeners();
        if (this.pageSize > 0) this.renderPagination();
    },

    renderHeader() {
        const thead = document.getElementById('table-head');
        const tr = document.createElement('tr');
        this.columns.forEach((col, i) => {
            const th = document.createElement('th');
            th.textContent = col;
            th.dataset.index = i;
            if (this.sortableColumns.includes(col)) {
                th.classList.add('sortable');
                th.addEventListener('click', () => this.sort(i));
            }
            tr.appendChild(th);
        });
        thead.appendChild(tr);
    },

    renderBody() {
        const tbody = document.getElementById('table-body');
        tbody.innerHTML = '';

        let data = this.getFilteredData();
        if (this.pageSize > 0) {
            const start = (this.currentPage - 1) * this.pageSize;
            data = data.slice(start, start + this.pageSize);
        }

        data.forEach(row => {
            const tr = document.createElement('tr');
            row.forEach(cell => {
                const td = document.createElement('td');
                td.textContent = cell === null ? '' : String(cell);
                tr.appendChild(td);
            });
            tbody.appendChild(tr);
        });
    },

    getFilteredData() {
        if (!this.filterText) return [...this.rows];
        const lower = this.filterText.toLowerCase();
        return this.rows.filter(row =>
            row.some(cell => String(cell).toLowerCase().includes(lower))
        );
    },

    sort(colIndex) {
        if (this.sortColumn === colIndex) {
            this.sortDirection = this.sortDirection === 'asc' ? 'desc' : 'asc';
        } else {
            this.sortColumn = colIndex;
            this.sortDirection = 'asc';
        }

        this.rows.sort((a, b) => {
            const aVal = a[colIndex];
            const bVal = b[colIndex];
            const cmp = aVal < bVal ? -1 : aVal > bVal ? 1 : 0;
            return this.sortDirection === 'asc' ? cmp : -cmp;
        });

        this.currentPage = 1;
        this.renderBody();
        this.updateSortIndicators();
        if (this.pageSize > 0) this.renderPagination();
    },

    updateSortIndicators() {
        document.querySelectorAll('th.sortable').forEach(th => {
            th.classList.remove('sort-asc', 'sort-desc');
            if (parseInt(th.dataset.index) === this.sortColumn) {
                th.classList.add(this.sortDirection === 'asc' ? 'sort-asc' : 'sort-desc');
            }
        });
    },

    setupEventListeners() {
        if (this.filterable) {
            const input = document.getElementById('filter-input');
            if (input) {
                input.addEventListener('input', (e) => {
                    this.filterText = e.target.value;
                    this.currentPage = 1;
                    this.renderBody();
                    if (this.pageSize > 0) this.renderPagination();
                });
            }
        }
    },

    renderPagination() {
        const container = document.getElementById('pagination');
        if (!container) return;

        const filtered = this.getFilteredData();
        const totalPages = Math.ceil(filtered.length / this.pageSize);

        container.innerHTML = '';

        const info = document.createElement('span');
        info.className = 'page-info';
        info.textContent = `Page ${this.currentPage} of ${totalPages} (${filtered.length} rows)`;
        container.appendChild(info);

        const nav = document.createElement('div');
        nav.className = 'page-nav';

        const prevBtn = document.createElement('button');
        prevBtn.textContent = 'Previous';
        prevBtn.disabled = this.currentPage === 1;
        prevBtn.addEventListener('click', () => { this.currentPage--; this.renderBody(); this.renderPagination(); });
        nav.appendChild(prevBtn);

        const nextBtn = document.createElement('button');
        nextBtn.textContent = 'Next';
        nextBtn.disabled = this.currentPage >= totalPages;
        nextBtn.addEventListener('click', () => { this.currentPage++; this.renderBody(); this.renderPagination(); });
        nav.appendChild(nextBtn);

        container.appendChild(nav);
    }
};

document.addEventListener('DOMContentLoaded', () => TableApp.init());

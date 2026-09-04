/* Columns arrive as {key, header, type, align?} objects. They used to be bare
 * key strings, which is why a column the author had labelled "Stage" was
 * headed STAGE_ID and a currency rendered as its raw JSON number. */
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

    /* Types whose values are read by comparison down a column, so they are
     * right-aligned unless the column says otherwise. */
    NUMERIC: ['integer', 'number', 'currency', 'percentage'],

    init() {
        this.renderHeader();
        this.renderBody();
        this.setupEventListeners();
        if (this.pageSize > 0) this.renderPagination();
    },

    alignOf(col) {
        return col.align || (this.NUMERIC.includes(col.type) ? 'right' : 'left');
    },

    /* The table is laid out fixed, so every column takes an equal share of the
     * frame unless it is told otherwise — which handed an id column the same
     * width as a subject line, truncating the text while the digits sat in
     * padding. These weights are relative, not absolute: a column's width is
     * its share of the total, so the columns always add up to the frame and the
     * table never needs to scroll sideways.
     *
     * Widths are emitted as plain percentages for a reason. A `calc()` mixing
     * `rem` against `100%` resolves to nothing usable inside the wrapper's
     * scroll container — the browser silently falls back to equal columns, so
     * the rule appears to work while doing nothing.
     *
     * The numeric types are weighted so a formatted value never has to be cut:
     * a truncated figure is worse than a truncated sentence, because
     * "1,200,0…" reads as a different number rather than as an abbreviation. */
    WEIGHT: { integer: 0.6, boolean: 0.8, percentage: 1.2, date: 1.6, currency: 1.6 },
    TEXT_WEIGHT: 1.5,

    /* The first free-text column carries a double share. In a table of records
     * that column is the record's name — the one a reader scans, and the only
     * one long enough to be worth the room. */
    columnWidths() {
        const weights = this.columns.map(col => this.WEIGHT[col.type] || this.TEXT_WEIGHT);
        const firstText = this.columns.findIndex(col => !this.WEIGHT[col.type]);
        if (firstText !== -1) weights[firstText] *= 2;

        const total = weights.reduce((sum, weight) => sum + weight, 0);
        if (!total) return this.columns.map(() => null);
        return weights.map(weight => `${((weight / total) * 100).toFixed(2)}%`);
    },

    renderHeader() {
        const thead = document.getElementById('table-head');
        const tr = document.createElement('tr');
        const widths = this.columnWidths();
        this.columns.forEach((col, i) => {
            const th = document.createElement('th');
            th.textContent = col.header;
            th.dataset.index = i;
            th.style.textAlign = this.alignOf(col);
            if (widths[i]) th.style.width = widths[i];
            th.setAttribute('scope', 'col');
            if (this.sortableColumns.includes(col.key)) {
                th.classList.add('sortable');
                th.tabIndex = 0;
                th.setAttribute('role', 'button');
                th.setAttribute('aria-label', `Sort by ${col.header}`);
                const sort = () => this.sort(i);
                th.addEventListener('click', sort);
                th.addEventListener('keydown', (e) => {
                    if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); sort(); }
                });
            }
            tr.appendChild(th);
        });
        thead.appendChild(tr);
    },

    /* Formatting is per column type. An unparseable value falls back to its
     * string form rather than rendering as "Invalid Date" or "NaN". */
    formatCell(value, type) {
        if (value === null || value === undefined || value === '') return '';
        switch (type) {
            case 'integer':
                return Number.isFinite(+value) ? (+value).toLocaleString() : String(value);
            case 'number':
            case 'currency': {
                if (!Number.isFinite(+value)) return String(value);
                /* No currency code exists on the column model, so this formats
                 * the magnitude and leaves the unit to the header. */
                return (+value).toLocaleString(undefined, {
                    minimumFractionDigits: 2,
                    maximumFractionDigits: 2,
                });
            }
            case 'percentage':
                return Number.isFinite(+value) ? `${(+value).toLocaleString()}%` : String(value);
            case 'date': {
                const d = new Date(value);
                return isNaN(d.getTime()) ? String(value) : d.toLocaleDateString();
            }
            case 'boolean':
                return value ? 'Yes' : 'No';
            default:
                return String(value);
        }
    },

    renderBody() {
        const tbody = document.getElementById('table-body');
        tbody.innerHTML = '';

        let data = this.getFilteredData();

        if (data.length === 0) {
            const tr = document.createElement('tr');
            const td = document.createElement('td');
            td.className = 'table-empty';
            td.colSpan = this.columns.length || 1;
            td.textContent = this.filterText
                ? `No rows match "${this.filterText}".`
                : 'No rows to show.';
            tr.appendChild(td);
            tbody.appendChild(tr);
            return;
        }

        if (this.pageSize > 0) {
            const start = (this.currentPage - 1) * this.pageSize;
            data = data.slice(start, start + this.pageSize);
        }

        data.forEach(row => {
            const tr = document.createElement('tr');
            row.forEach((cell, i) => {
                const col = this.columns[i] || { type: 'string' };
                const td = document.createElement('td');
                td.style.textAlign = this.alignOf(col);
                /* Every cell's content goes inside this span, links included.
                 * The ellipsis is applied here rather than to the cell because
                 * a `td` is not a block box and does not clip its own overflow
                 * — and a link left outside the span was the one cell type that
                 * could still push the table past its frame. The full value
                 * stays reachable on hover. */
                const span = document.createElement('span');
                span.className = 'cell-text';
                if (col.type === 'link' && cell) {
                    const a = document.createElement('a');
                    a.href = String(cell);
                    a.textContent = String(cell);
                    a.rel = 'noopener noreferrer';
                    a.target = '_blank';
                    span.appendChild(a);
                    span.title = String(cell);
                } else {
                    const text = this.formatCell(cell, col.type);
                    span.textContent = text;
                    if (text) span.title = text;
                }
                td.appendChild(span);
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

    /* Compares by column type: numbers and dates numerically, everything else
     * by locale string. Nulls always sort last regardless of direction, so an
     * empty cell never displaces real data at the top of the column. */
    compare(a, b, type) {
        const aNull = a === null || a === undefined || a === '';
        const bNull = b === null || b === undefined || b === '';
        if (aNull && bNull) return 0;
        if (aNull) return 1;
        if (bNull) return -1;

        if (type === 'date') {
            const at = new Date(a).getTime(), bt = new Date(b).getTime();
            if (!isNaN(at) && !isNaN(bt)) return at - bt;
        } else if (this.NUMERIC.includes(type)) {
            const an = +a, bn = +b;
            if (Number.isFinite(an) && Number.isFinite(bn)) return an - bn;
        } else if (type === 'boolean') {
            return (a === b) ? 0 : (a ? 1 : -1);
        }
        return String(a).localeCompare(String(b), undefined, { numeric: true });
    },

    sort(colIndex) {
        if (this.sortColumn === colIndex) {
            this.sortDirection = this.sortDirection === 'asc' ? 'desc' : 'asc';
        } else {
            this.sortColumn = colIndex;
            this.sortDirection = 'asc';
        }

        const type = (this.columns[colIndex] || {}).type;
        const dir = this.sortDirection === 'asc' ? 1 : -1;
        /* Sort a copy: mutating `rows` in place made the artifact's own data
         * order depend on what the viewer had last clicked. */
        this.rows = [...this.rows].sort((a, b) =>
            dir * this.compare(a[colIndex], b[colIndex], type)
        );

        this.currentPage = 1;
        this.renderBody();
        this.updateSortIndicators();
        if (this.pageSize > 0) this.renderPagination();
    },

    updateSortIndicators() {
        document.querySelectorAll('th.sortable').forEach(th => {
            th.classList.remove('sort-asc', 'sort-desc');
            th.removeAttribute('aria-sort');
            if (parseInt(th.dataset.index) === this.sortColumn) {
                const asc = this.sortDirection === 'asc';
                th.classList.add(asc ? 'sort-asc' : 'sort-desc');
                th.setAttribute('aria-sort', asc ? 'ascending' : 'descending');
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
        const totalPages = Math.max(1, Math.ceil(filtered.length / this.pageSize));

        container.innerHTML = '';

        const info = document.createElement('span');
        info.className = 'page-info';
        const rowWord = filtered.length === 1 ? 'row' : 'rows';
        info.textContent = `Page ${this.currentPage} of ${totalPages} (${filtered.length} ${rowWord})`;
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

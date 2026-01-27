# Documentation Publishing Issues

## Summary

Issues encountered when creating and publishing documentation pages with the SSG system. **All issues have been resolved.**

## Environment

- SystemPrompt version: 0.0.14
- Profile: local
- Template location: `/web/templates/`

---

## RESOLVED Issues

### Issue 1: Template Registry Empty During Publish Job (FIXED)

**Status:** Resolved after rebuild

**Original Problem:**
The `publish_content` job logged `Template registry initialized templates=0` even though templates were registered via CLI.

**Resolution:** Issue was resolved after `just build` - templates now load correctly and all 33 documentation pages are generated.

---

### Issue 2: Slug Validation (FIXED)

**Status:** Resolved - nested paths now supported

**Original Problem:**
Slug validation rejected slashes, preventing hierarchical URLs like `/documentation/getting-started/installation`.

**Resolution:** Nested slugs are now supported. URLs can use path structure.

---

### Issue 3: Template Path Mismatch (FIXED)

**Status:** Resolved

**Original Problem:**
Templates expected at `/web/templates/` but project structure had them at `/services/web/templates/`.

**Resolution:** Templates should be placed in `/web/templates/` directory.

---

### Issue 4: CSS/JS Assets Not Copied to Dist (FIXED)

**Status:** Resolved after rebuild

**Original Problem:**
CSS and JS files in `/storage/files/css/` and `/storage/files/js/` were not automatically copied to `/web/dist/css/` and `/web/dist/js/` during the `publish_content` job.

**Resolution:** Assets are now correctly copied to `/web/dist/` during publish pipeline.

---

## Current Working State

| Metric | Value |
|--------|-------|
| Documentation pages generated | 33 |
| CSS files in dist | 9 |
| JS files in dist | 3 |
| Sitemap entries | 38 |

All documentation pages publish correctly with proper asset copying.

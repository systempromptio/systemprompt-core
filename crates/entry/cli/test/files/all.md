# Files CLI Test Results

**Date:** 2026-01-13
**Profile:** /var/www/html/tyingshoelaces/.systemprompt/profiles/local/profile.yaml

## Summary

| Command | Status |
|---------|--------|
| files config | ✅ PASS |
| files validate | ✅ PASS |
| files list | ✅ PASS |
| files show | ✅ PASS |
| files upload | ✅ PASS |
| files delete | ✅ PASS |
| files content list | ✅ PASS |
| files content link | ✅ PASS |
| files content unlink | ✅ PASS |
| files content featured | ✅ PASS |
| files ai list | ✅ PASS |
| files ai count | ✅ PASS |

## Test Details

### files config

```bash
systemprompt --non-interactive files config --json
```

Output: Shows file upload configuration including storage paths, allowed types, and size limits.

### files validate

```bash
systemprompt --non-interactive files validate /path/to/file --json
```

Output: Validates file type and size before upload, returns allowed/disallowed status.

### files list

```bash
systemprompt --non-interactive files list --json
```

Output: Returns paginated list of files with id, path, public_url, mime_type, size_bytes, ai_content, created_at.

### files show

```bash
systemprompt --non-interactive files show <FILE_ID> --json
```

Output: Returns detailed file information including metadata, checksums, and associated context/session/trace IDs.

### files upload

```bash
systemprompt --non-interactive files upload /path/to/file --context <CONTEXT_ID> --user <USER_ID> --json
```

Output: Uploads file to storage and creates database record. Returns file_id, path, public_url, size_bytes, mime_type, checksum_sha256.

### files delete

```bash
systemprompt --non-interactive files delete <FILE_ID> --json
```

Output: Deletes file from storage and database. Returns confirmation message.

### files content list

```bash
systemprompt --non-interactive files content list <CONTENT_ID> --json
```

Output: Lists all files linked to a content item with file_id, path, mime_type, role, display_order.

### files content link

```bash
systemprompt --non-interactive files content link <FILE_ID> --content <CONTENT_ID> --role <ROLE> --json
```

Roles: featured, attachment, inline, og-image, thumbnail

Output: Links file to content with specified role. Returns confirmation.

### files content unlink

```bash
systemprompt --non-interactive files content unlink <FILE_ID> --content <CONTENT_ID> --json
```

Output: Removes file-content association. Returns confirmation.

### files content featured

```bash
systemprompt --non-interactive files content featured <CONTENT_ID> --json
```

Output: Returns the featured image for the content item, or message if none set.

### files ai list

```bash
systemprompt --non-interactive files ai list --json
```

Output: Lists AI-generated images with pagination.

### files ai count

```bash
systemprompt --non-interactive files ai count --user <USER_ID> --json
```

Output: Returns count of AI-generated images for the specified user.

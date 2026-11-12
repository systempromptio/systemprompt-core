-- An upload is stored under files/uploads/ and served at
-- <url_prefix>/uploads/..., the storage id without its leading files/
-- segment. Rows recorded with the whole storage id appended to the prefix
-- are rewritten to the served address.

UPDATE files
SET public_url = left(public_url, length(public_url) - length(path)) || substr(path, 7)
WHERE path LIKE 'files/uploads/%'
  AND right(public_url, length(path) + 1) = '/' || path;

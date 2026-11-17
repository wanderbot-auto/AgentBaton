# Exact log archives

All command stdout/stderr, including failed runs, is preserved byte-for-byte in `logs/*.gz`. `LOGS.json` records original filenames, raw and compressed SHA-256 values, byte counts and retained local raw paths. No output whitespace or failure text was normalized. Gzip is used because raw tool output contains trailing spaces and terminal blank lines; source/document whitespace checks remain unchanged.

Use Python `gzip.open(path, 'rt')` to inspect an archive. Original uncompressed files remain under ignored `artifacts/history-export/raw-evidence-1aabbc/`.

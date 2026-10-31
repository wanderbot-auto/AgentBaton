# Progress

- First binding now requires nonempty exact argv plus stable PID/start/UID/PPID/PGID across two readonly metadata samples. Initial absent/replaced/terminal/unknown observations never append a verified Graph record.
- Repeated collection requires the independently retained initial metadata. Only exact live argv, explicit absent/replaced identity, or the same bound terminal identity can preserve the existing record. Unknown or empty live argv and any observed invalid nonempty argv remain refusals. No new binding is created by a repeat.
- Cmdline reads stop at 8193 bytes to classify the original 8192-byte limit; diagnostics retain only closed grammar kind/byte count and metadata, never raw argv. Each identity/class is written once privately, at most 12 closed observation records. No new stop, join, cancel or acceptance predicate is added; `stop_proved` remains false for every observation class.
- Complete exact-source inverse restores the original entire harness hash, preserving all existing business/SQL/schema/zero-table/timeout/cleanup source. Original weak `old=None/current=None` append behavior is no longer accepted.
- 39 pure controls pass: new 11 process-decision/mocked-collector/source controls plus existing bound-readonly-wait and finite diagnostics. No actual process, SQLite, browser, Core or CI was run by this author.

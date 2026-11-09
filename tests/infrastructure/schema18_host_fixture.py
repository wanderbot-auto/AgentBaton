"""Exact current-host schema inverse; historical source pins remain in callers."""

def restore_schema17_host(text):
    current = ('        let version: u32 = db\n'
               '            .pragma_query_value(None, "user_version", |r| r.get(0))\n'
               '            .unwrap();\n'
               '        assert_eq!(version, 18);\n')
    if type(text) is not str or text.count(current) != 1:
        raise AssertionError('exact current schema18 host reader required')
    return text.replace(current, current.replace('version, 18', 'version, 17'), 1)

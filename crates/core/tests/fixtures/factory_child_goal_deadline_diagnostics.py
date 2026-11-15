# Included only in the generated one-second deadline fixture. Observations do
# not authorize a retry, establish an owned Goal, or change any tool assertion.
import time

DEADLINE_STAGES = frozenset({
    'parent_task_status', 'delegate_child', 'child_planning_status',
    'submit_child_plan', 'child_execution_status', 'execute_child_goal',
})
DEADLINE_CODES = frozenset({
    'WORKSPACE_BUSY', 'RESOURCE_LIMIT', 'DEADLINE_EXCEEDED',
    'PROVIDER_DISCONNECTED', 'OUTCOME_UNKNOWN', 'BUDGET_UNKNOWN',
    'INPUT_CHANGED', 'AUTH_REQUIRED', 'STALE_REVISION', 'CONFIG_CONFLICT',
    'PERMISSION_DENIED', 'NOT_FOUND', 'INVALID_REQUEST', 'INTERNAL',
})
DEADLINE_RPC_CODES = {
    -32700: 'parse_error', -32600: 'invalid_request',
    -32601: 'method_not_found', -32602: 'invalid_params',
    -32603: 'internal_error',
}
_deadline_origin = time.monotonic_ns()
_deadline_active = None
_deadline_records = []


def deadline_error_fields(value, *, rpc=False):
    error = value if type(value) is dict else {}
    code = error.get('code')
    if rpc:
        code = DEADLINE_RPC_CODES.get(code, 'other') if type(code) is int else 'invalid'
    else:
        code = code if type(code) is str and code in DEADLINE_CODES else 'other'
    flag = error.get('retryable')
    retryable = ('true' if flag else 'false') if type(flag) is bool else ('absent' if 'retryable' not in error else 'invalid')
    effect = error.get('side_effect_state')
    effect = effect if type(effect) is str and effect in {'not_started', 'confirmed', 'unknown'} else ('absent' if 'side_effect_state' not in error else 'invalid')
    return {'code': code, 'retryable': retryable, 'side_effect_state': effect}


def deadline_record(event, layer, reply_class, fields=None):
    if _deadline_active is None or len(_deadline_records) >= 32:
        return
    record = {'stage': _deadline_active, 'event': event, 'layer': layer,
              'reply_class': reply_class,
              'elapsed_ms': max(0, (time.monotonic_ns() - _deadline_origin) // 1000000)}
    if fields is not None:
        record.update(fields)
    _deadline_records.append(record)
    # Best effort instrumentation must not replace the original tool failure.
    # Atomic overwrite prevents a killed Provider leaving a partial last record.
    try:
        raw = json.dumps({'version': 1, 'dimension': 'generated_parent_mcp_stages',
                          'records': _deadline_records}, separators=(',', ':')).encode()
        if len(raw) > 16384:
            return
        path = 'reports/goal-deadline-parent-stages.json'
        fd = os.open(path + '.tmp', os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
        with os.fdopen(fd, 'wb') as writer:
            os.fchmod(writer.fileno(), 0o600)
            writer.write(raw)
        os.replace(path + '.tmp', path)
    except OSError:
        pass


def deadline_envelope_reply(value, *, rpc):
    if _deadline_active is None:
        return
    layer = 'jsonrpc' if rpc else 'actual_tool'
    if type(value) is not dict or ('result' in value) == ('error' in value):
        deadline_record('reply', layer, 'malformed')
    elif 'error' in value:
        deadline_record('reply', layer, 'error', deadline_error_fields(value['error'], rpc=rpc))
    else:
        deadline_record('reply', layer, 'result')


def deadline_tools_reply(value):
    if _deadline_active is None:
        return
    if type(value) is not dict:
        deadline_record('reply', 'tools_call', 'malformed')
        return
    flag = value.get('isError')
    if 'isError' in value and type(flag) is not bool:
        kind = 'malformed'
    elif flag is True:
        kind = 'error'
    elif type(value.get('content')) is list:
        kind = 'result'
    else:
        kind = 'malformed'
    deadline_record('reply', 'tools_call', kind)


def deadline_tool(stage, name, args):
    global _deadline_active
    assert stage in DEADLINE_STAGES
    _deadline_active = stage
    deadline_record('before', 'boundary', 'unobserved')
    try:
        result = tool(name, args)
    except BaseException as error:
        # Exception names/messages and the rejected wire body are never copied.
        classes = {json.JSONDecodeError: 'json_decode_error', AssertionError: 'assertion',
                   BrokenPipeError: 'broken_pipe', EOFError: 'eof', OSError: 'os_error',
                   KeyError: 'key_error', TypeError: 'type_error', ValueError: 'value_error'}
        deadline_record('after', 'local', classes.get(type(error), 'other_failure'))
        raise
    else:
        deadline_record('after', 'boundary', 'returned')
        return result
    finally:
        _deadline_active = None

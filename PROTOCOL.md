# Relapse socket protocol v1

Relapse talks to an executor backend over a local Unix stream socket. This document specifies the wire format the UI sends and expects.

> This is the UI's protocol specification, not a claim about Relapse's private
> internals. Relapse does not publish an official IPC document; the demo
> backend exists so the UI can be developed and reviewed without an executor.

## Transport

- AF_UNIX, SOCK_STREAM.
- Default path: `$XDG_RUNTIME_DIR/relapse-$UID.sock`, or `RELAPSE_SOCKET`.
- The socket is created with mode `0600`.
- Newline-delimited UTF-8 JSON. One object per line, no pretty printing.
- Maximum frame size: 4 MiB.

## Envelope

Every frame is an object. There are three shapes:

### Request (client → backend)

```json
{
  "v": 1,
  "id": "4f6c1d2a9b30",
  "op": "execute",
  "params": {
    "instance": "4242",
    "source": "print(\"Hello World, \",getexecutorname())"
  }
}
```

### Response (backend → client)

```json
{
  "v": 1,
  "id": "4f6c1d2a9b30",
  "ok": true,
  "result": {
    "execution": "ex_9a8b7c6d",
    "instance": "4242"
  }
}
```

Errors use the same envelope with `ok: false`:

```json
{
  "v": 1,
  "id": "4f6c1d2a9b30",
  "ok": false,
  "error": {
    "code": "no_instance",
    "message": "No attached instance to execute in"
  }
}
```

### Event (backend → client, unsolicited)

```json
{
  "v": 1,
  "event": "output",
  "data": {
    "execution": "ex_9a8b7c6d",
    "instance": "4242",
    "stream": "stdout",
    "text": "Hello World, Relapse\n"
  }
}
```

Events have no `id`. A client must tolerate unknown events and fields.

## Handshake

The first frame on every connection is `hello`.

```json
{ "v": 1, "id": "a1", "op": "hello",
  "params": { "client": "relapse-ui", "version": "1.0.0", "protocol": 1 } }
```

```json
{ "v": 1, "id": "a1", "ok": true,
  "result": {
    "server": "relapse-demo",
    "executor": "Relapse 2.4.1 (demo)",
    "protocol": 1,
    "pid": 11922,
    "features": ["instances", "attach", "detach", "execute", "execute_file"],
    "demo": true
  } }
```

The backend may reject older clients with `unsupported_version`. The UI
displays the negotiated server/protocol in Settings and an **Attached** state in
the bottom bar once at least one instance is attached. The bundled demo backend
starts with every instance detached, so that state only appears after an
explicit attach.

## Operations

| `op` | `params` | Success `result` |
| --- | --- | --- |
| `hello` | `client`, `version`, `protocol` | `server`, `executor`, `protocol`, `pid`, `features` |
| `ping` | — | `pong`, `time` |
| `instances` | — | `instances`: list of instance objects |
| `attach` | `instance` (string id) | `instances` |
| `detach` | `instance` (string id) | `instances` |
| `execute` | `source`, optional `instance`, optional `name` | `execution`, `instance` |
| `execute_file` | `path`, optional `instance` | `execution`, `instance` |

### Instance object

```json
{
  "id": "4242",
  "pid": 4242,
  "name": "RobloxPlayerBeta",
  "place": "Baseplate",
  "attached": true,
  "executor": "Relapse 2.4.1 (demo)"
}
```

`id` is opaque to the client. `pid` is informational. `attached` controls
whether the instance can execute code. The UI labels the `attach` operation
**Inject**; the wire operation remains `attach`.

### `execute` and `execute_file`

The backend acknowledges the request immediately, then streams output events.
`execute` sends source text inline; `execute_file` sends a filesystem path that
the backend reads. Both return an `execution` id used to correlate events.

Both operations accept an optional `redirect_console` boolean. When true, the
backend should stream output events back to the UI console; when false, output
belongs in the client's own console.

Execution events:

```json
{ "v": 1, "event": "output",
  "data": { "execution": "ex_9a8b7c6d", "instance": "4242",
            "stream": "stdout", "text": "Hello World, Relapse\n" } }
```

```json
{ "v": 1, "event": "execution_finished",
  "data": { "execution": "ex_9a8b7c6d", "instance": "4242",
            "ok": true, "duration_ms": 100 } }
```

`stream` is `stdout` or `stderr`. Output may be split across multiple frames.
An execution may emit zero or more output events before
`execution_finished`.

### `instances_changed`

Backends broadcast this when the set of instances changes independently of a
request (for example, Roblox opens or closes).

```json
{ "v": 1, "event": "instances_changed",
  "data": { "instances": [ /* instance objects */ ] } }
```

The client also accepts `instances` in attach/detach responses, so a backend may
send both; the UI treats them as idempotent snapshots.

## Error codes

| Code | Meaning |
| --- | --- |
| `bad_request` | Malformed frame, missing id, wrong params type |
| `unknown_op` | Unsupported operation |
| `unsupported_version` | Protocol version mismatch |
| `unknown_instance` | `instance` does not exist |
| `no_instance` | No attached instance is available |
| `not_attached` | Target instance is not attached |
| `file_error` | `execute_file` could not read the path |
| `frame_too_large` | Frame exceeded the 4 MiB limit |
| `timeout` | Client-side: no response within 8 seconds |

## Example transcript

```
→ {"v":1,"id":"a1","op":"hello","params":{"client":"relapse-ui","version":"1.0.0","protocol":1}}
← {"v":1,"id":"a1","ok":true,"result":{"server":"relapse-demo","protocol":1,"executor":"Relapse 2.4.1 (demo)"}}
→ {"v":1,"id":"a2","op":"instances","params":{}}
← {"v":1,"id":"a2","ok":true,"result":{"instances":[{"id":"4242","pid":4242,"name":"RobloxPlayerBeta","place":"Baseplate","attached":true}]}}
→ {"v":1,"id":"a3","op":"execute","params":{"instance":"4242","source":"print(\"hi\")","redirect_console":true}}
← {"v":1,"id":"a3","ok":true,"result":{"execution":"ex_9a8b7c6d","instance":"4242"}}
← {"v":1,"event":"output","data":{"execution":"ex_9a8b7c6d","instance":"4242","stream":"stdout","text":"Hello World, Relapse\n"}}
← {"v":1,"event":"execution_finished","data":{"execution":"ex_9a8b7c6d","instance":"4242","ok":true,"duration_ms":100}}
```

## Implementation notes

- the UI client registers a pending callback per request id, sends on a
  background thread, and marshals responses/events back to the GTK main loop.
- Requests time out after 8 seconds. A dropped connection fails all pending
  callbacks with `disconnected`.
- The client retries the connection every 2 seconds and re-runs `hello` on each
  new socket.
- The UI trace in Settings shows every frame, with long script sources elided.

The demo backend never parses or evaluates Lua. It validates the frame, picks an
attached instance, waits briefly, and emits simulated output. It deliberately
starts with no instances, so execution returns `no_instance` until a real
backend reports an attached client.

The UI only accepts `.lua`, `.luau` and `.txt` files for scripts. Backends may
apply their own policy in `execute_file`.

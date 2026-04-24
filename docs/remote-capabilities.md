# Remote Debugging Capability Negotiation

## Overview

When a client connects to a remote Soroban debugger server, both sides now exchange capability metadata during the handshake. This allows incompatibilities to be detected **at connection time** rather than later when operations are attempted.

## How It Works

### Connection Handshake Sequence

```
Client                                    Server
  |                                         |
  |--- Connect (TCP) ------------------>  |
  |                                         |
  |--- Handshake Request                   |
  |    (client_name, client_version,       |
  |     protocol_version,                  |
  |     required_capabilities) -------->  |
  |                                         |
  |                                    [Validate protocol version]
  |                                    [Build server capabilities]
  |                                    [Check compatibility]
  |                                         |
  |<--- Handshake Response                 |
  |     (server_version,                   |
  |      server_capabilities,              |
  |      negotiated_features) ----------   |
  |                                         |
  |--- Authenticate (if token) -------->  |
  |                                         |
  |<--- Auth Response -------------------- |
  |                                         |
  | [Ready for operations]                 |
  |                                         |
```

### Capability Exchange

During handshake, the client can declare which capabilities it requires. The server responds with:

1. **Its full capability set** - what features this server build supports
2. **Validation result** - whether the client's required capabilities are met

If the server lacks any required capability, the connection is rejected immediately with a clear error message.

## Supported Capabilities

The following capabilities can be negotiated:

| Capability | Description |
|---|---|
| `conditional_breakpoints` | Supports conditional and hit-count breakpoints |
| `source_breakpoints` | Supports source-level (DWARF) breakpoints via `ResolveSourceBreakpoints` |
| `evaluate` | Supports the `Evaluate` request for expression inspection |
| `tls` | Supports TLS-encrypted connections |
| `token_auth` | Supports token-based authentication |
| `session_lifecycle` | Supports heartbeat/idle-timeout negotiation |
| `repeat_execution` | Supports repeat execution via `repeat_count` |
| `symbolic_analysis` | Supports the symbolic analysis command |
| `snapshot_loading` | Supports loading network snapshots via `LoadSnapshot` |
| `dynamic_trace_events` | Supports the `GetEvents` / DynamicTrace command |

## Error Scenarios

### Scenario 1: Client Requires Feature Server Doesn't Support

**Client declares:** `required_capabilities: { evaluate: true, snapshot_loading: true }`

**Server supports:** `{ evaluate: true, snapshot_loading: false, ... }`

**Result:** Connection rejected at handshake with error:
```
Server is missing required capabilities [snapshot_loading]. 
Upgrade the server or disable these features on the client.
```

### Scenario 2: Server Supports Feature Client Doesn't Know About

**Server advertises:** `{ new_feature: true, ... }`

**Client behavior:** Ignores unknown capabilities (forward compatible)

**Result:** Connection succeeds; client simply won't use the unknown feature

### Scenario 3: Both Support All Required Features

**Client declares:** `required_capabilities: { evaluate: true }`

**Server supports:** `{ evaluate: true, ... }`

**Result:** Connection succeeds; operations proceed normally

## Backward Compatibility

- **Old clients connecting to new servers:** If the client doesn't send `required_capabilities`, the server treats it as having no requirements and accepts the connection.
- **New clients connecting to old servers:** If the server doesn't advertise capabilities, the client treats it as supporting nothing optional and may reject operations.

## Usage Examples

### Rust Client

```rust
use soroban_debugger::client::RemoteClient;
use soroban_debugger::server::protocol::ServerCapabilities;

// Create a client that requires specific capabilities
let mut config = RemoteClientConfig::default();
config.required_capabilities = Some(ServerCapabilities {
    evaluate: true,
    snapshot_loading: true,
    ..Default::default()
});

let mut client = RemoteClient::connect_with_config(
    "127.0.0.1:8000",
    None,
    config,
)?;

// If server doesn't support evaluate, this fails at handshake:
// "Server is missing required capabilities [evaluate]"
```

### Checking Negotiated Capabilities

After successful handshake, the client can inspect what the server supports:

```rust
if let Some(caps) = &client.negotiated_capabilities {
    if caps.evaluate {
        // Safe to use evaluate
        let (result, _) = client.evaluate("some_expr", None)?;
    } else {
        eprintln!("Server does not support expression evaluation");
    }
}
```

## Troubleshooting

### "Server is missing required capabilities"

**Cause:** The server build doesn't support a feature the client needs.

**Solutions:**
1. Upgrade the server to a newer version that supports the feature
2. Disable the feature requirement on the client side
3. Check the server's capability list to see what it does support

### "Unexpected response to Handshake"

**Cause:** The server is too old and doesn't understand the new capability negotiation protocol.

**Solution:** Upgrade the server to a version that supports capability negotiation (this feature was added in version X.Y.Z).

## Implementation Details

### Server-Side

The server builds its capability set in `ServerCapabilities::current()`:

```rust
pub fn current() -> Self {
    Self {
        conditional_breakpoints: true,
        source_breakpoints: true,
        evaluate: true,
        tls: true,
        token_auth: true,
        session_lifecycle: true,
        repeat_execution: true,
        symbolic_analysis: false, // opt-in
        snapshot_loading: true,
        dynamic_trace_events: true,
    }
}
```

During handshake, if the client declares required capabilities, the server checks:

```rust
let missing = required.unsupported_by(&our_caps);
if !missing.is_empty() {
    // Reject with IncompatibleCapabilities response
}
```

### Client-Side

The client stores negotiated capabilities after handshake:

```rust
pub struct RemoteClient {
    // ...
    pub negotiated_capabilities: Option<ServerCapabilities>,
}
```

Optional methods guard themselves:

```rust
pub fn evaluate(&mut self, expression: &str, frame_id: Option<u64>) -> Result<...> {
    self.require_capability("evaluate")?;
    // ... proceed with operation
}
```

## Protocol Messages

### Handshake Request

```json
{
  "type": "Handshake",
  "client_name": "rust-remote-client",
  "client_version": "1.0.0",
  "protocol_min": 1,
  "protocol_max": 1,
  "required_capabilities": {
    "evaluate": true,
    "snapshot_loading": true,
    "conditional_breakpoints": false,
    ...
  }
}
```

### Handshake Response (Success)

```json
{
  "type": "HandshakeAck",
  "server_name": "soroban-debug",
  "server_version": "1.0.0",
  "protocol_min": 1,
  "protocol_max": 1,
  "selected_version": 1,
  "server_capabilities": {
    "evaluate": true,
    "snapshot_loading": true,
    "conditional_breakpoints": true,
    ...
  }
}
```

### Handshake Response (Capability Mismatch)

```json
{
  "type": "IncompatibleCapabilities",
  "message": "Server does not support required capabilities: snapshot_loading. Upgrade the server or disable these features on the client.",
  "missing_capabilities": ["snapshot_loading"],
  "server_capabilities": {
    "evaluate": true,
    "snapshot_loading": false,
    ...
  }
}
```

## See Also

- [Remote Debugging Guide](./remote-debugging.md)
- [Feature Matrix](./feature-matrix.md)
- [Protocol Reference](../src/server/protocol.rs)

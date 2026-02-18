# MCP GDB Server

[日本語](README_ja.md)

An MCP (Model Context Protocol) server for controlling GDB. Supports both local and remote debugging with any GDB executable.

## Features

- **Flexible GDB Support**: Works with any GDB executable (gdb, gdb-multiarch, arm-none-eabi-gdb, etc.)
- **Remote Debugging**: TCP connections (QEMU, JTAG debuggers, etc.) and serial port connections
- **GDB/MI Protocol**: Reliable communication using GDB Machine Interface
- **Rich Toolset**: Breakpoints, execution control, memory operations, register access, and more

## Installation

### Build Requirements

- Rust 1.70 or later
- GDB (any variant)

### Build

```bash
cd mcp-gdb-server
cargo build --release
```

The binary will be generated at `target/release/mcp-gdb-server`.

## Usage

### Configuration for Claude Desktop

Add the following to `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "gdb": {
      "command": "/path/to/mcp-gdb-server"
    }
  }
}
```

## Guide for LLMs

### When to Use This Server

Use this server when the user wants to:
- Debug a program (local or remote)
- Set breakpoints and step through code
- Inspect memory, registers, or variables
- Connect to QEMU or embedded devices via GDB

### Typical Workflows

#### Local Debugging Workflow

1. **Start GDB session**: Use `gdb_start` with the appropriate GDB path
2. **Load program**: Use `gdb_load_file` to load the executable
3. **Set breakpoints**: Use `gdb_break_insert` at key locations (function names, line numbers, or addresses)
4. **Run program**: Use `gdb_run` to start execution
5. **Step through code**: Use `gdb_step`, `gdb_next`, `gdb_stepi`, or `gdb_nexti`
6. **Inspect state**: Use `gdb_evaluate`, `gdb_registers_list`, `gdb_memory_read`
7. **Continue or stop**: Use `gdb_continue` to resume or `gdb_stop` to end

#### Remote Debugging Workflow (QEMU, JTAG, etc.)

1. **Start GDB session**: Use `gdb_start` with appropriate GDB (e.g., `gdb-multiarch` for cross-architecture)
2. **Connect to target**: Use `gdb_target_connect` with host:port or serial device
3. **Debug normally**: Set breakpoints, step, inspect - same as local debugging
4. **Note**: `gdb_run` typically not needed for remote targets - use `gdb_continue` instead

### Important Notes for LLMs

- **Architecture matters**: For cross-architecture debugging, specify the `architecture` parameter in `gdb_start` (e.g., "arm", "aarch64", "riscv:rv64")
- **Instruction-level stepping**: Use `gdb_stepi`/`gdb_nexti` when debugging without source symbols
- **Remote targets**: The program must already be running or paused on the remote side
- **Check status**: Use `gdb_status` to verify the current state before operations
- **Address format**: Use `*0xADDRESS` format for address-based breakpoints (e.g., `*0x80000000`)

### Common Pitfalls

- Don't try to `gdb_run` on a remote target - use `gdb_continue` instead
- Wait for the target to stop before using step commands
- Ensure the correct architecture is set when cross-debugging

## Available Tools

#### Session Management

| Tool | Description |
|------|-------------|
| `gdb_start` | Start a GDB session (specify gdb_path, optionally architecture) |
| `gdb_stop` | Stop the GDB session |
| `gdb_status` | Get current session status |

#### File Operations

| Tool | Description |
|------|-------------|
| `gdb_load_file` | Load an executable file |

#### Remote Debugging

| Tool | Description |
|------|-------------|
| `gdb_target_connect` | Connect to a remote target (TCP/serial) |
| `gdb_target_disconnect` | Disconnect from the remote target |

#### Breakpoints

| Tool | Description |
|------|-------------|
| `gdb_break_insert` | Set a breakpoint |
| `gdb_break_delete` | Delete a breakpoint |
| `gdb_break_list` | List all breakpoints |
| `gdb_break_toggle` | Enable/disable a breakpoint |

#### Execution Control

| Tool | Description |
|------|-------------|
| `gdb_run` | Start the program |
| `gdb_continue` | Continue execution |
| `gdb_next` | Step over (source level) |
| `gdb_step` | Step into (source level) |
| `gdb_nexti` | Step over (instruction level) |
| `gdb_stepi` | Step into (instruction level) |
| `gdb_finish` | Step out |
| `gdb_interrupt` | Interrupt execution |

#### Stack & Threads

| Tool | Description |
|------|-------------|
| `gdb_stack_list` | Display call stack |
| `gdb_stack_select` | Select a stack frame |
| `gdb_stack_info` | Get current frame info |
| `gdb_thread_list` | List all threads |
| `gdb_thread_select` | Select a thread |

#### Memory & Registers

| Tool | Description |
|------|-------------|
| `gdb_memory_read` | Read memory |
| `gdb_memory_write` | Write to memory |
| `gdb_registers_list` | List registers with names and values |
| `gdb_register_set` | Set register value |

#### Variables & Evaluation

| Tool | Description |
|------|-------------|
| `gdb_evaluate` | Evaluate an expression (e.g., `$pc`, `variable_name`) |
| `gdb_variable_info` | Get variable details |

#### Advanced Operations

| Tool | Description |
|------|-------------|
| `gdb_raw_command` | Execute a raw GDB/MI command |

## Usage Examples

### Local Debugging

```
1. gdb_start gdb_path="gdb"
2. gdb_load_file file_path="/path/to/binary"
3. gdb_break_insert location="main"
4. gdb_run
5. gdb_next
6. gdb_evaluate expression="variable"
```

### Remote Debugging (QEMU)

```
# First, start QEMU with GDB server:
# qemu-system-aarch64 -M virt -nographic -kernel /path/to/kernel -s -S

1. gdb_start gdb_path="gdb-multiarch" architecture="aarch64"
2. gdb_target_connect target_type="remote" host="localhost" port=1234
3. gdb_break_insert location="*0x40000000"
4. gdb_continue
5. gdb_stepi count=10
6. gdb_registers_list
```

### Remote Debugging (Embedded via Serial JTAG)

```
1. gdb_start gdb_path="arm-none-eabi-gdb" architecture="arm"
2. gdb_target_connect serial_port="/dev/ttyUSB0" baud_rate=115200
3. gdb_break_insert location="main"
4. gdb_continue
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      LLM (Claude, etc.)                      │
└──────────────────────────┬──────────────────────────────────┘
                            │ MCP Protocol (JSON-RPC)
                            ▼
┌─────────────────────────────────────────────────────────────┐
│                   MCP GDB Server                            │
│  ┌─────────────────┐  ┌─────────────────┐                  │
│  │  MCP Handler    │  │  Tool Handlers  │                  │
│  └────────┬────────┘  └────────┬────────┘                  │
│           └─────────────────────┘                          │
│                           │                                 │
│  ┌────────────────────────▼────────────────────────────┐   │
│  │              GDB Client                              │   │
│  │  ┌──────────────┐  ┌──────────────────────────────┐ │   │
│  │  │ MI Parser    │  │ Process Management           │ │   │
│  │  └──────────────┘  └──────────────────────────────┘ │   │
│  └─────────────────────────┬───────────────────────────┘   │
└────────────────────────────┼───────────────────────────────┘
                              │ GDB/MI Protocol
                              ▼
               ┌──────────────────────────────┐
               │   GDB (any variant)          │
               │   ┌────────────────────┐     │
               │   │ Target              │     │
               │   │ (Local/Remote)      │     │
               │   └────────────────────┘     │
               └──────────────────────────────┘
```

## License

MIT License

## Contributing

Pull requests are welcome. For major changes, please open an issue first to discuss what you would like to change.

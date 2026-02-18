//! MCP Tool Definitions for GDB Debugging

use serde::{Deserialize, Serialize};

/// MCP Tool definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// Tool: Start GDB session
pub fn tool_start_gdb() -> ToolDefinition {
    ToolDefinition {
        name: "gdb_start".to_string(),
        description: "Start a new GDB debugging session. Can specify gdb-multiarch path and architecture for cross-architecture debugging.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "gdb_path": {
                    "type": "string",
                    "description": "Path to GDB executable (default: gdb-multiarch)"
                },
                "architecture": {
                    "type": "string",
                    "description": "Target architecture (e.g., arm, aarch64, riscv, mips)"
                }
            },
            "required": []
        }),
    }
}

/// Tool: Stop GDB session
pub fn tool_stop_gdb() -> ToolDefinition {
    ToolDefinition {
        name: "gdb_stop".to_string(),
        description: "Stop the current GDB debugging session and clean up resources.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    }
}

/// Tool: Load executable
pub fn tool_load_file() -> ToolDefinition {
    ToolDefinition {
        name: "gdb_load_file".to_string(),
        description: "Load an executable file and its symbol table into GDB for debugging.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Path to the executable file to debug"
                }
            },
            "required": ["file_path"]
        }),
    }
}

/// Tool: Connect to remote target
pub fn tool_target_connect() -> ToolDefinition {
    ToolDefinition {
        name: "gdb_target_connect".to_string(),
        description: "Connect to a remote debugging target via TCP or serial port. Supports both 'remote' and 'extended-remote' connection types.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "target_type": {
                    "type": "string",
                    "enum": ["remote", "extended-remote"],
                    "description": "Type of remote connection"
                },
                "host": {
                    "type": "string",
                    "description": "Hostname or IP address for TCP connection"
                },
                "port": {
                    "type": "integer",
                    "description": "TCP port number for remote connection"
                },
                "serial_port": {
                    "type": "string",
                    "description": "Serial device path (e.g., /dev/ttyUSB0)"
                },
                "baud_rate": {
                    "type": "integer",
                    "description": "Baud rate for serial connection"
                }
            },
            "required": []
        }),
    }
}

/// Tool: Disconnect from target
pub fn tool_target_disconnect() -> ToolDefinition {
    ToolDefinition {
        name: "gdb_target_disconnect".to_string(),
        description: "Disconnect from the current remote debugging target.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    }
}

/// Tool: Set breakpoint
pub fn tool_break_insert() -> ToolDefinition {
    ToolDefinition {
        name: "gdb_break_insert".to_string(),
        description: "Insert a breakpoint at the specified location. Location can be a function name, line number (file:line), or address.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "location": {
                    "type": "string",
                    "description": "Breakpoint location (function name, file:line, or *address)"
                },
                "temporary": {
                    "type": "boolean",
                    "description": "Create a temporary breakpoint that is deleted after being hit"
                },
                "condition": {
                    "type": "string",
                    "description": "Optional condition expression for conditional breakpoint"
                },
                "ignore_count": {
                    "type": "integer",
                    "description": "Number of times to ignore this breakpoint before stopping"
                }
            },
            "required": ["location"]
        }),
    }
}

/// Tool: Delete breakpoint
pub fn tool_break_delete() -> ToolDefinition {
    ToolDefinition {
        name: "gdb_break_delete".to_string(),
        description: "Delete one or all breakpoints.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "number": {
                    "type": "string",
                    "description": "Breakpoint number to delete (omit to delete all breakpoints)"
                }
            },
            "required": []
        }),
    }
}

/// Tool: List breakpoints
pub fn tool_break_list() -> ToolDefinition {
    ToolDefinition {
        name: "gdb_break_list".to_string(),
        description: "List all breakpoints in the current debugging session.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    }
}

/// Tool: Enable/Disable breakpoint
pub fn tool_break_toggle() -> ToolDefinition {
    ToolDefinition {
        name: "gdb_break_toggle".to_string(),
        description: "Enable or disable a breakpoint.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "number": {
                    "type": "string",
                    "description": "Breakpoint number"
                },
                "enabled": {
                    "type": "boolean",
                    "description": "true to enable, false to disable"
                }
            },
            "required": ["number", "enabled"]
        }),
    }
}

/// Tool: Set watchpoint
pub fn tool_watch_insert() -> ToolDefinition {
    ToolDefinition {
        name: "gdb_watch_insert".to_string(),
        description: "Set a watchpoint on a variable or memory location. Watchpoints trigger when the watched location is read, written, or accessed.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "location": {
                    "type": "string",
                    "description": "Variable name or memory expression to watch (e.g., 'counter', '*ptr', '&myvar')"
                },
                "watch_type": {
                    "type": "string",
                    "enum": ["write", "read", "access"],
                    "description": "Type of watchpoint: 'write' (trigger on write), 'read' (trigger on read), 'access' (trigger on read or write)"
                }
            },
            "required": ["location"]
        }),
    }
}

/// Tool: Delete watchpoint
pub fn tool_watch_delete() -> ToolDefinition {
    ToolDefinition {
        name: "gdb_watch_delete".to_string(),
        description: "Delete a watchpoint by its number.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "number": {
                    "type": "string",
                    "description": "Watchpoint number to delete"
                }
            },
            "required": ["number"]
        }),
    }
}

/// Tool: Run/Start execution
pub fn tool_run() -> ToolDefinition {
    ToolDefinition {
        name: "gdb_run".to_string(),
        description: "Start program execution from the beginning. For remote debugging, this typically loads and starts the program.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "args": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Command line arguments to pass to the program"
                }
            },
            "required": []
        }),
    }
}

/// Tool: Continue execution
pub fn tool_continue() -> ToolDefinition {
    ToolDefinition {
        name: "gdb_continue".to_string(),
        description: "Continue program execution from the current stopped state.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    }
}

/// Tool: Step over
pub fn tool_next() -> ToolDefinition {
    ToolDefinition {
        name: "gdb_next".to_string(),
        description: "Step over the current line (execute without entering function calls).".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "count": {
                    "type": "integer",
                    "description": "Number of lines to step over"
                }
            },
            "required": []
        }),
    }
}

/// Tool: Step into
pub fn tool_step() -> ToolDefinition {
    ToolDefinition {
        name: "gdb_step".to_string(),
        description: "Step into the current line (enter function calls).".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "count": {
                    "type": "integer",
                    "description": "Number of steps to perform"
                }
            },
            "required": []
        }),
    }
}

/// Tool: Step out
pub fn tool_finish() -> ToolDefinition {
    ToolDefinition {
        name: "gdb_finish".to_string(),
        description: "Step out of the current function (continue until function returns).".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    }
}

/// Tool: Step one instruction
pub fn tool_stepi() -> ToolDefinition {
    ToolDefinition {
        name: "gdb_stepi".to_string(),
        description: "Step one machine instruction (assembly level step into). Useful for debugging without source symbols.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "count": {
                    "type": "integer",
                    "description": "Number of instructions to step"
                }
            },
            "required": []
        }),
    }
}

/// Tool: Next one instruction
pub fn tool_nexti() -> ToolDefinition {
    ToolDefinition {
        name: "gdb_nexti".to_string(),
        description: "Step one machine instruction, stepping over function calls (assembly level next).".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "count": {
                    "type": "integer",
                    "description": "Number of instructions to step over"
                }
            },
            "required": []
        }),
    }
}

/// Tool: Interrupt execution
pub fn tool_interrupt() -> ToolDefinition {
    ToolDefinition {
        name: "gdb_interrupt".to_string(),
        description: "Interrupt the running program (send SIGINT to target).".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    }
}

/// Tool: Get stack trace
pub fn tool_stack_list() -> ToolDefinition {
    ToolDefinition {
        name: "gdb_stack_list".to_string(),
        description: "Get the current call stack (backtrace).".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "low_frame": {
                    "type": "integer",
                    "description": "Starting frame number"
                },
                "high_frame": {
                    "type": "integer",
                    "description": "Ending frame number (-1 for all frames)"
                }
            },
            "required": []
        }),
    }
}

/// Tool: Select frame
pub fn tool_stack_select() -> ToolDefinition {
    ToolDefinition {
        name: "gdb_stack_select".to_string(),
        description: "Select a specific stack frame for inspection.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "level": {
                    "type": "integer",
                    "description": "Frame level to select (0 = innermost)"
                }
            },
            "required": ["level"]
        }),
    }
}

/// Tool: Get frame info
pub fn tool_stack_info() -> ToolDefinition {
    ToolDefinition {
        name: "gdb_stack_info".to_string(),
        description: "Get information about the currently selected stack frame.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    }
}

/// Tool: List threads
pub fn tool_thread_list() -> ToolDefinition {
    ToolDefinition {
        name: "gdb_thread_list".to_string(),
        description: "List all threads in the debugged program.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    }
}

/// Tool: Select thread
pub fn tool_thread_select() -> ToolDefinition {
    ToolDefinition {
        name: "gdb_thread_select".to_string(),
        description: "Select a specific thread for debugging.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "thread_id": {
                    "type": "string",
                    "description": "Thread ID to select"
                }
            },
            "required": ["thread_id"]
        }),
    }
}

/// Tool: Read memory
pub fn tool_memory_read() -> ToolDefinition {
    ToolDefinition {
        name: "gdb_memory_read".to_string(),
        description: "Read memory contents from the target at the specified address.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "address": {
                    "type": "string",
                    "description": "Memory address to read from (can be expression like &variable)"
                },
                "count": {
                    "type": "integer",
                    "description": "Number of bytes to read"
                }
            },
            "required": ["address"]
        }),
    }
}

/// Tool: Write memory
pub fn tool_memory_write() -> ToolDefinition {
    ToolDefinition {
        name: "gdb_memory_write".to_string(),
        description: "Write data to memory at the specified address.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "address": {
                    "type": "string",
                    "description": "Memory address to write to"
                },
                "data": {
                    "type": "string",
                    "description": "Hex bytes to write (e.g., '0x90 0x90')"
                }
            },
            "required": ["address", "data"]
        }),
    }
}

/// Tool: Evaluate expression
pub fn tool_evaluate() -> ToolDefinition {
    ToolDefinition {
        name: "gdb_evaluate".to_string(),
        description: "Evaluate a C/C++ expression in the current context and return its value.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "expression": {
                    "type": "string",
                    "description": "Expression to evaluate (e.g., 'variable', 'ptr->field', 'array[0]')"
                }
            },
            "required": ["expression"]
        }),
    }
}

/// Tool: List registers
pub fn tool_registers_list() -> ToolDefinition {
    ToolDefinition {
        name: "gdb_registers_list".to_string(),
        description: "List all CPU registers and their current values.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    }
}

/// Tool: Set register
pub fn tool_register_set() -> ToolDefinition {
    ToolDefinition {
        name: "gdb_register_set".to_string(),
        description: "Set the value of a CPU register.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "register": {
                    "type": "string",
                    "description": "Register name (e.g., 'pc', 'sp', 'r0')"
                },
                "value": {
                    "type": "string",
                    "description": "Value to set (can be expression)"
                }
            },
            "required": ["register", "value"]
        }),
    }
}

/// Tool: Get variable info
pub fn tool_variable_info() -> ToolDefinition {
    ToolDefinition {
        name: "gdb_variable_info".to_string(),
        description: "Get detailed information about a variable including its type and children (for structs/arrays).".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Variable name to inspect"
                },
                "depth": {
                    "type": "integer",
                    "description": "Depth of children to retrieve for complex types"
                }
            },
            "required": ["name"]
        }),
    }
}

/// Tool: Get session status
pub fn tool_status() -> ToolDefinition {
    ToolDefinition {
        name: "gdb_status".to_string(),
        description: "Get the current GDB session status including connection state, current thread/frame, and running state.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    }
}

/// Tool: Execute raw GDB command
pub fn tool_raw_command() -> ToolDefinition {
    ToolDefinition {
        name: "gdb_raw_command".to_string(),
        description: "Execute a raw GDB/MI command directly. Use for advanced operations not covered by other tools.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "GDB/MI command to execute (without leading '-')"
                }
            },
            "required": ["command"]
        }),
    }
}

/// Get all available tools
pub fn get_all_tools() -> Vec<ToolDefinition> {
    vec![
        tool_start_gdb(),
        tool_stop_gdb(),
        tool_load_file(),
        tool_target_connect(),
        tool_target_disconnect(),
        tool_break_insert(),
        tool_break_delete(),
        tool_break_list(),
        tool_break_toggle(),
        tool_watch_insert(),
        tool_watch_delete(),
        tool_run(),
        tool_continue(),
        tool_next(),
        tool_step(),
        tool_stepi(),
        tool_nexti(),
        tool_finish(),
        tool_interrupt(),
        tool_stack_list(),
        tool_stack_select(),
        tool_stack_info(),
        tool_thread_list(),
        tool_thread_select(),
        tool_memory_read(),
        tool_memory_write(),
        tool_evaluate(),
        tool_registers_list(),
        tool_register_set(),
        tool_variable_info(),
        tool_status(),
        tool_raw_command(),
    ]
}

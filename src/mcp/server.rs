//! MCP Server Handler Implementation

use crate::gdb::{GdbClient, GdbConfig, GdbSessionState, Register, WatchpointType};
use crate::mcp::protocol::*;
use crate::mcp::tools::get_all_tools;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// GDB MCP Server
pub struct GdbMcpServer {
    client: Arc<RwLock<Option<GdbClient>>>,
}

impl GdbMcpServer {
    pub fn new() -> Self {
        Self {
            client: Arc::new(RwLock::new(None)),
        }
    }

    /// Get server info
    pub fn get_info(&self) -> InitializeResult {
        InitializeResult {
            protocol_version: "2024-11-05".to_string(),
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability {
                    list_changed: Some(false),
                }),
                ..Default::default()
            },
            server_info: Implementation {
                name: "mcp-gdb-server".to_string(),
                version: "0.1.0".to_string(),
            },
            instructions: Some(
                "GDB MCP Server for debugging programs with support for gdb-multiarch and remote debugging.\n\n\
                 Start with 'gdb_start', load a program with 'gdb_load_file', set breakpoints with 'gdb_break_insert', \
                 and control execution with 'gdb_run', 'gdb_continue', 'gdb_next', 'gdb_step', and 'gdb_finish'.\n\n\
                 For remote debugging (embedded systems, QEMU), use 'gdb_target_connect' with host:port or serial device.".to_string()
            ),
        }
    }

    /// Handle initialize request
    pub async fn handle_initialize(&self, _params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let info = self.get_info();
        Ok(serde_json::to_value(info)?)
    }

    /// Handle tools/list request
    pub async fn handle_tools_list(&self) -> Result<serde_json::Value> {
        let all_tools = get_all_tools();
        let tools: Vec<Tool> = all_tools
            .iter()
            .map(|t| Tool {
                name: t.name.clone(),
                description: Some(t.description.clone()),
                input_schema: t.input_schema.clone(),
            })
            .collect();

        let result = ListToolsResult {
            tools,
            next_cursor: None,
        };

        Ok(serde_json::to_value(result)?)
    }

    /// Handle tools/call request
    pub async fn handle_tools_call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params = params.ok_or_else(|| anyhow::anyhow!("Missing params"))?;
        let request: CallToolRequest = serde_json::from_value(params)?;
        
        debug!("Calling tool: {}", request.name);
        
        let result = match request.name.as_str() {
            "gdb_start" => self.handle_start(request.arguments).await,
            "gdb_stop" => self.handle_stop().await,
            "gdb_load_file" => self.handle_load_file(request.arguments).await,
            "gdb_target_connect" => self.handle_target_connect(request.arguments).await,
            "gdb_target_disconnect" => self.handle_target_disconnect().await,
            "gdb_break_insert" => self.handle_break_insert(request.arguments).await,
            "gdb_break_delete" => self.handle_break_delete(request.arguments).await,
            "gdb_break_list" => self.handle_break_list().await,
            "gdb_break_toggle" => self.handle_break_toggle(request.arguments).await,
            "gdb_watch_insert" => self.handle_watch_insert(request.arguments).await,
            "gdb_watch_delete" => self.handle_watch_delete(request.arguments).await,
            "gdb_run" => self.handle_run(request.arguments).await,
            "gdb_continue" => self.handle_continue().await,
            "gdb_next" => self.handle_next(request.arguments).await,
            "gdb_step" => self.handle_step(request.arguments).await,
            "gdb_stepi" => self.handle_stepi(request.arguments).await,
            "gdb_nexti" => self.handle_nexti(request.arguments).await,
            "gdb_finish" => self.handle_finish().await,
            "gdb_interrupt" => self.handle_interrupt().await,
            "gdb_stack_list" => self.handle_stack_list().await,
            "gdb_stack_select" => self.handle_stack_select(request.arguments).await,
            "gdb_stack_info" => self.handle_stack_info().await,
            "gdb_thread_list" => self.handle_thread_list().await,
            "gdb_thread_select" => self.handle_thread_select(request.arguments).await,
            "gdb_memory_read" => self.handle_memory_read(request.arguments).await,
            "gdb_memory_write" => self.handle_memory_write(request.arguments).await,
            "gdb_evaluate" => self.handle_evaluate(request.arguments).await,
            "gdb_registers_list" => self.handle_registers_list().await,
            "gdb_register_set" => self.handle_register_set(request.arguments).await,
            "gdb_variable_info" => self.handle_variable_info(request.arguments).await,
            "gdb_status" => self.handle_status().await,
            "gdb_raw_command" => self.handle_raw_command(request.arguments).await,
            _ => Ok(CallToolResult::error_text(format!("Unknown tool: {}", request.name))),
        };

        Ok(serde_json::to_value(result?)?)
    }

    // ========================================================================
    // Tool Handlers
    // ========================================================================

    async fn handle_start(&self, args: Option<serde_json::Map<String, serde_json::Value>>) -> Result<CallToolResult> {
        let gdb_path = args
            .as_ref()
            .and_then(|a| a.get("gdb_path"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "gdb-multiarch".to_string());

        let architecture = args
            .and_then(|a| a.get("architecture").and_then(|v| v.as_str()).map(|s| s.to_string()));

        info!("Starting GDB session with: {}", gdb_path);

        let mut guard = self.client.write().await;
        if guard.is_some() {
            return Ok(CallToolResult::error_text("GDB session already running. Use gdb_stop first."));
        }

        let config = GdbConfig {
            gdb_path,
            architecture,
            ..Default::default()
        };

        let mut client = GdbClient::new(config);
        client.start()?;

        *guard = Some(client);

        Ok(CallToolResult::text("GDB session started successfully. Use gdb_load_file to load a program, or gdb_target_connect for remote debugging."))
    }

    async fn handle_stop(&self) -> Result<CallToolResult> {
        info!("Stopping GDB session");

        let mut guard = self.client.write().await;
        if let Some(mut client) = guard.take() {
            client.stop()?;
            Ok(CallToolResult::text("GDB session stopped successfully."))
        } else {
            Ok(CallToolResult::error_text("No GDB session is running."))
        }
    }

    async fn handle_load_file(&self, args: Option<serde_json::Map<String, serde_json::Value>>) -> Result<CallToolResult> {
        let file_path = args
            .and_then(|a| a.get("file_path").and_then(|v| v.as_str()).map(|s| s.to_string()))
            .ok_or_else(|| anyhow::anyhow!("file_path is required"))?;

        info!("Loading file: {}", file_path);

        let guard = self.client.read().await;
        let client = guard.as_ref().ok_or_else(|| anyhow::anyhow!("GDB session not started"))?;
        
        // We need mutable access, so we'll need to restructure this
        drop(guard);
        
        let mut guard = self.client.write().await;
        let client = guard.as_mut().ok_or_else(|| anyhow::anyhow!("GDB session not started"))?;
        client.file_exec_and_symbols(&file_path)?;

        Ok(CallToolResult::text(format!("Loaded executable: {}", file_path)))
    }

    async fn handle_target_connect(&self, args: Option<serde_json::Map<String, serde_json::Value>>) -> Result<CallToolResult> {
        let target_type = args.as_ref()
            .and_then(|a| a.get("target_type").and_then(|v| v.as_str()).map(|s| s.to_string()));
        let host = args.as_ref().and_then(|a| a.get("host").and_then(|v| v.as_str()).map(|s| s.to_string()));
        let port = args.as_ref().and_then(|a| a.get("port").and_then(|v| v.as_u64()).map(|n| n as u16));
        let serial_port = args.as_ref().and_then(|a| a.get("serial_port").and_then(|v| v.as_str()).map(|s| s.to_string()));

        let target_string = if let (Some(h), Some(p)) = (host, port) {
            format!("{}:{}", h, p)
        } else if let Some(sp) = serial_port {
            sp
        } else {
            return Ok(CallToolResult::error_text("Either host:port or serial_port must be specified."));
        };

        let is_extended = target_type.as_deref() == Some("extended-remote");
        info!("Connecting to {} target: {}", if is_extended { "extended-remote" } else { "remote" }, target_string);

        let mut guard = self.client.write().await;
        let client = guard.as_mut().ok_or_else(|| anyhow::anyhow!("GDB session not started"))?;

        if is_extended {
            client.target_connect_extended_remote(&target_string)?;
        } else {
            client.target_connect_remote(&target_string)?;
        }

        Ok(CallToolResult::text(format!("Connected to remote target: {}", target_string)))
    }

    async fn handle_target_disconnect(&self) -> Result<CallToolResult> {
        let mut guard = self.client.write().await;
        let client = guard.as_mut().ok_or_else(|| anyhow::anyhow!("GDB session not started"))?;
        client.target_disconnect()?;
        Ok(CallToolResult::text("Disconnected from remote target."))
    }

    async fn handle_break_insert(&self, args: Option<serde_json::Map<String, serde_json::Value>>) -> Result<CallToolResult> {
        let location = args.as_ref()
            .and_then(|a| a.get("location").and_then(|v| v.as_str()).map(|s| s.to_string()))
            .ok_or_else(|| anyhow::anyhow!("location is required"))?;
        let temporary = args.as_ref().and_then(|a| a.get("temporary").and_then(|v| v.as_bool())).unwrap_or(false);
        let condition = args.as_ref().and_then(|a| a.get("condition").and_then(|v| v.as_str()).map(|s| s.to_string()));

        info!("Inserting breakpoint at: {}", location);

        let mut guard = self.client.write().await;
        let client = guard.as_mut().ok_or_else(|| anyhow::anyhow!("GDB session not started"))?;
        
        let bp = client.break_insert(&location, temporary, condition.as_deref())?;
        
        Ok(CallToolResult::success(vec![
            Content::text(format!("Breakpoint {} inserted at {}", bp.number, location)),
            Content::text(serde_json::to_string_pretty(&bp)?),
        ]))
    }

    async fn handle_break_delete(&self, args: Option<serde_json::Map<String, serde_json::Value>>) -> Result<CallToolResult> {
        let number = args.and_then(|a| a.get("number").and_then(|v| v.as_str()).map(|s| s.to_string()));

        let mut guard = self.client.write().await;
        let client = guard.as_mut().ok_or_else(|| anyhow::anyhow!("GDB session not started"))?;

        if let Some(n) = number {
            client.break_delete(&n)?;
            Ok(CallToolResult::text(format!("Breakpoint {} deleted.", n)))
        } else {
            client.send_command("break-delete")?;
            Ok(CallToolResult::text("All breakpoints deleted."))
        }
    }

    async fn handle_break_list(&self) -> Result<CallToolResult> {
        let mut guard = self.client.write().await;
        let client = guard.as_mut().ok_or_else(|| anyhow::anyhow!("GDB session not started"))?;
        
        let breakpoints = client.break_list()?;
        Ok(CallToolResult::text(serde_json::to_string_pretty(&breakpoints)?))
    }

    async fn handle_break_toggle(&self, args: Option<serde_json::Map<String, serde_json::Value>>) -> Result<CallToolResult> {
        let number = args.as_ref()
            .and_then(|a| a.get("number").and_then(|v| v.as_str()).map(|s| s.to_string()))
            .ok_or_else(|| anyhow::anyhow!("number is required"))?;
        let enabled = args.as_ref()
            .and_then(|a| a.get("enabled").and_then(|v| v.as_bool()))
            .ok_or_else(|| anyhow::anyhow!("enabled is required"))?;

        let mut guard = self.client.write().await;
        let client = guard.as_mut().ok_or_else(|| anyhow::anyhow!("GDB session not started"))?;

        if enabled {
            client.break_enable(&number)?;
            Ok(CallToolResult::text(format!("Breakpoint {} enabled.", number)))
        } else {
            client.break_disable(&number)?;
            Ok(CallToolResult::text(format!("Breakpoint {} disabled.", number)))
        }
    }

    async fn handle_watch_insert(&self, args: Option<serde_json::Map<String, serde_json::Value>>) -> Result<CallToolResult> {
        let location = args.as_ref()
            .and_then(|a| a.get("location").and_then(|v| v.as_str()).map(|s| s.to_string()))
            .ok_or_else(|| anyhow::anyhow!("location is required"))?;
        
        let watch_type = args.as_ref()
            .and_then(|a| a.get("watch_type").and_then(|v| v.as_str()))
            .map(|s| match s {
                "read" => WatchpointType::Read,
                "access" => WatchpointType::Access,
                _ => WatchpointType::Write,
            })
            .unwrap_or(WatchpointType::Write);

        info!("Inserting {:?} watchpoint at: {}", watch_type, location);

        let mut guard = self.client.write().await;
        let client = guard.as_mut().ok_or_else(|| anyhow::anyhow!("GDB session not started"))?;
        
        let wp = client.watch_insert(watch_type.clone(), &location)?;
        
        let type_str = match watch_type {
            WatchpointType::Write => "write",
            WatchpointType::Read => "read",
            WatchpointType::Access => "access",
        };
        
        Ok(CallToolResult::success(vec![
            Content::text(format!("{} watchpoint {} inserted at {}", type_str, wp.number, location)),
            Content::text(serde_json::to_string_pretty(&wp)?),
        ]))
    }

    async fn handle_watch_delete(&self, args: Option<serde_json::Map<String, serde_json::Value>>) -> Result<CallToolResult> {
        let number = args.as_ref()
            .and_then(|a| a.get("number").and_then(|v| v.as_str()).map(|s| s.to_string()))
            .ok_or_else(|| anyhow::anyhow!("number is required"))?;

        let mut guard = self.client.write().await;
        let client = guard.as_mut().ok_or_else(|| anyhow::anyhow!("GDB session not started"))?;
        
        client.break_delete(&number)?;
        Ok(CallToolResult::text(format!("Watchpoint {} deleted.", number)))
    }

    async fn handle_run(&self, args: Option<serde_json::Map<String, serde_json::Value>>) -> Result<CallToolResult> {
        let program_args = args.and_then(|a| a.get("args").and_then(|v| v.as_array()).map(|arr| {
            arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect::<Vec<_>>()
        }));

        let mut guard = self.client.write().await;
        let client = guard.as_mut().ok_or_else(|| anyhow::anyhow!("GDB session not started"))?;

        if let Some(ref a) = program_args {
            let args_str = a.join(" ");
            client.send_command(&format!("exec-arguments {}", args_str))?;
        }

        client.exec_run()?;
        Ok(CallToolResult::text("Program started. Waiting for stop event..."))
    }

    async fn handle_continue(&self) -> Result<CallToolResult> {
        let mut guard = self.client.write().await;
        let client = guard.as_mut().ok_or_else(|| anyhow::anyhow!("GDB session not started"))?;
        client.exec_continue()?;
        Ok(CallToolResult::text("Program running. Waiting for stop event..."))
    }

    async fn handle_next(&self, args: Option<serde_json::Map<String, serde_json::Value>>) -> Result<CallToolResult> {
        let count = args.and_then(|a| a.get("count").and_then(|v| v.as_u64())).unwrap_or(1);

        let mut guard = self.client.write().await;
        let client = guard.as_mut().ok_or_else(|| anyhow::anyhow!("GDB session not started"))?;

        for _ in 0..count {
            client.exec_next()?;
        }
        Ok(CallToolResult::text(format!("Stepped over {} line(s).", count)))
    }

    async fn handle_step(&self, args: Option<serde_json::Map<String, serde_json::Value>>) -> Result<CallToolResult> {
        let count = args.and_then(|a| a.get("count").and_then(|v| v.as_u64())).unwrap_or(1);

        let mut guard = self.client.write().await;
        let client = guard.as_mut().ok_or_else(|| anyhow::anyhow!("GDB session not started"))?;

        for _ in 0..count {
            client.exec_step()?;
        }
        Ok(CallToolResult::text(format!("Stepped into {} line(s).", count)))
    }

    async fn handle_finish(&self) -> Result<CallToolResult> {
        let mut guard = self.client.write().await;
        let client = guard.as_mut().ok_or_else(|| anyhow::anyhow!("GDB session not started"))?;
        client.exec_finish()?;
        Ok(CallToolResult::text("Stepping out of function..."))
    }

    async fn handle_stepi(&self, args: Option<serde_json::Map<String, serde_json::Value>>) -> Result<CallToolResult> {
        let count = args.as_ref()
            .and_then(|a| a.get("count").and_then(|v| v.as_u64()))
            .unwrap_or(1);

        let mut guard = self.client.write().await;
        let client = guard.as_mut().ok_or_else(|| anyhow::anyhow!("GDB session not started"))?;
        
        for _ in 0..count {
            client.exec_step_instruction()?;
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        
        let pc = client.data_evaluate_expression("$pc")?;
        Ok(CallToolResult::text(format!("Stepped {} instruction(s). PC = {}", count, pc)))
    }

    async fn handle_nexti(&self, args: Option<serde_json::Map<String, serde_json::Value>>) -> Result<CallToolResult> {
        let count = args.as_ref()
            .and_then(|a| a.get("count").and_then(|v| v.as_u64()))
            .unwrap_or(1);

        let mut guard = self.client.write().await;
        let client = guard.as_mut().ok_or_else(|| anyhow::anyhow!("GDB session not started"))?;
        
        for _ in 0..count {
            client.exec_next_instruction()?;
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        
        let pc = client.data_evaluate_expression("$pc")?;
        Ok(CallToolResult::text(format!("Stepped {} instruction(s). PC = {}", count, pc)))
    }

    async fn handle_interrupt(&self) -> Result<CallToolResult> {
        let mut guard = self.client.write().await;
        let client = guard.as_mut().ok_or_else(|| anyhow::anyhow!("GDB session not started"))?;
        client.exec_interrupt()?;
        Ok(CallToolResult::text("Program interrupted."))
    }

    async fn handle_stack_list(&self) -> Result<CallToolResult> {
        let mut guard = self.client.write().await;
        let client = guard.as_mut().ok_or_else(|| anyhow::anyhow!("GDB session not started"))?;
        let frames = client.stack_list_frames()?;
        Ok(CallToolResult::text(serde_json::to_string_pretty(&frames)?))
    }

    async fn handle_stack_select(&self, args: Option<serde_json::Map<String, serde_json::Value>>) -> Result<CallToolResult> {
        let level = args.as_ref()
            .and_then(|a| a.get("level").and_then(|v| v.as_u64()))
            .ok_or_else(|| anyhow::anyhow!("level is required"))?;

        let mut guard = self.client.write().await;
        let client = guard.as_mut().ok_or_else(|| anyhow::anyhow!("GDB session not started"))?;
        client.stack_select_frame(level)?;
        Ok(CallToolResult::text(format!("Selected frame {}.", level)))
    }

    async fn handle_stack_info(&self) -> Result<CallToolResult> {
        let mut guard = self.client.write().await;
        let client = guard.as_mut().ok_or_else(|| anyhow::anyhow!("GDB session not started"))?;
        
        if let Some(frame) = client.stack_info_frame()? {
            Ok(CallToolResult::text(serde_json::to_string_pretty(&frame)?))
        } else {
            Ok(CallToolResult::error_text("No frame information available."))
        }
    }

    async fn handle_thread_list(&self) -> Result<CallToolResult> {
        let mut guard = self.client.write().await;
        let client = guard.as_mut().ok_or_else(|| anyhow::anyhow!("GDB session not started"))?;
        let threads = client.thread_list_ids()?;
        Ok(CallToolResult::text(serde_json::to_string_pretty(&threads)?))
    }

    async fn handle_thread_select(&self, args: Option<serde_json::Map<String, serde_json::Value>>) -> Result<CallToolResult> {
        let thread_id = args.as_ref()
            .and_then(|a| a.get("thread_id").and_then(|v| v.as_str()).map(|s| s.to_string()))
            .ok_or_else(|| anyhow::anyhow!("thread_id is required"))?;

        let mut guard = self.client.write().await;
        let client = guard.as_mut().ok_or_else(|| anyhow::anyhow!("GDB session not started"))?;
        client.thread_select(&thread_id)?;
        Ok(CallToolResult::text(format!("Selected thread {}.", thread_id)))
    }

    async fn handle_memory_read(&self, args: Option<serde_json::Map<String, serde_json::Value>>) -> Result<CallToolResult> {
        let address = args.as_ref()
            .and_then(|a| a.get("address").and_then(|v| v.as_str()).map(|s| s.to_string()))
            .ok_or_else(|| anyhow::anyhow!("address is required"))?;
        let count = args.and_then(|a| a.get("count").and_then(|v| v.as_u64())).unwrap_or(16);

        let mut guard = self.client.write().await;
        let client = guard.as_mut().ok_or_else(|| anyhow::anyhow!("GDB session not started"))?;
        let mem = client.data_read_memory(&address, count)?;
        Ok(CallToolResult::text(serde_json::to_string_pretty(&mem)?))
    }

    async fn handle_memory_write(&self, args: Option<serde_json::Map<String, serde_json::Value>>) -> Result<CallToolResult> {
        let address = args.as_ref()
            .and_then(|a| a.get("address").and_then(|v| v.as_str()).map(|s| s.to_string()))
            .ok_or_else(|| anyhow::anyhow!("address is required"))?;
        let data = args.as_ref()
            .and_then(|a| a.get("data").and_then(|v| v.as_str()).map(|s| s.to_string()))
            .ok_or_else(|| anyhow::anyhow!("data is required"))?;

        let mut guard = self.client.write().await;
        let client = guard.as_mut().ok_or_else(|| anyhow::anyhow!("GDB session not started"))?;
        client.send_command(&format!("data-write-memory-bytes {} {}", address, data))?;
        Ok(CallToolResult::text(format!("Wrote data to address {}.", address)))
    }

    async fn handle_evaluate(&self, args: Option<serde_json::Map<String, serde_json::Value>>) -> Result<CallToolResult> {
        let expression = args.as_ref()
            .and_then(|a| a.get("expression").and_then(|v| v.as_str()).map(|s| s.to_string()))
            .ok_or_else(|| anyhow::anyhow!("expression is required"))?;

        let mut guard = self.client.write().await;
        let client = guard.as_mut().ok_or_else(|| anyhow::anyhow!("GDB session not started"))?;
        let value = client.data_evaluate_expression(&expression)?;
        Ok(CallToolResult::text(format!("{} = {}", expression, value)))
    }

    async fn handle_registers_list(&self) -> Result<CallToolResult> {
        let mut guard = self.client.write().await;
        let client = guard.as_mut().ok_or_else(|| anyhow::anyhow!("GDB session not started"))?;
        
        // Get register names
        let names = client.data_list_register_names()?;
        
        // Get register values
        let values = client.data_list_register_values()?;
        
        // Combine names and values
        let mut registers = Vec::new();
        let mut value_map = std::collections::HashMap::new();
        for reg in &values {
            value_map.insert(reg.number, &reg.value);
        }
        
        for (i, name) in names.iter().enumerate() {
            if !name.is_empty() {
                let value = value_map.get(&(i as u64)).map(|s| (*s).clone()).unwrap_or_else(|| "<unavailable>".to_string());
                registers.push(Register {
                    number: i as u64,
                    name: name.clone(),
                    value,
                });
            }
        }
        
        Ok(CallToolResult::text(serde_json::to_string_pretty(&registers)?))
    }

    async fn handle_register_set(&self, args: Option<serde_json::Map<String, serde_json::Value>>) -> Result<CallToolResult> {
        let register = args.as_ref()
            .and_then(|a| a.get("register").and_then(|v| v.as_str()).map(|s| s.to_string()))
            .ok_or_else(|| anyhow::anyhow!("register is required"))?;
        let value = args.as_ref()
            .and_then(|a| a.get("value").and_then(|v| v.as_str()).map(|s| s.to_string()))
            .ok_or_else(|| anyhow::anyhow!("value is required"))?;

        let mut guard = self.client.write().await;
        let client = guard.as_mut().ok_or_else(|| anyhow::anyhow!("GDB session not started"))?;
        client.send_command(&format!("gdb-set ${}={}", register, value))?;
        Ok(CallToolResult::text(format!("Set register {} = {}.", register, value)))
    }

    async fn handle_variable_info(&self, args: Option<serde_json::Map<String, serde_json::Value>>) -> Result<CallToolResult> {
        let name = args.as_ref()
            .and_then(|a| a.get("name").and_then(|v| v.as_str()).map(|s| s.to_string()))
            .ok_or_else(|| anyhow::anyhow!("name is required"))?;

        let mut guard = self.client.write().await;
        let client = guard.as_mut().ok_or_else(|| anyhow::anyhow!("GDB session not started"))?;
        
        let var = client.var_create(&name, None)?;
        let value = client.var_evaluate_expression(&name)?;
        
        Ok(CallToolResult::success(vec![
            Content::text(format!("{} = {}", name, value)),
            Content::text(serde_json::to_string_pretty(&var)?),
        ]))
    }

    async fn handle_status(&self) -> Result<CallToolResult> {
        let guard = self.client.read().await;
        let status = if let Some(client) = guard.as_ref() {
            client.state()
        } else {
            GdbSessionState::default()
        };
        Ok(CallToolResult::text(serde_json::to_string_pretty(&status)?))
    }

    async fn handle_raw_command(&self, args: Option<serde_json::Map<String, serde_json::Value>>) -> Result<CallToolResult> {
        let command = args.as_ref()
            .and_then(|a| a.get("command").and_then(|v| v.as_str()).map(|s| s.to_string()))
            .ok_or_else(|| anyhow::anyhow!("command is required"))?;

        let mut guard = self.client.write().await;
        let client = guard.as_mut().ok_or_else(|| anyhow::anyhow!("GDB session not started"))?;
        
        let response = client.send_command(&command)?;
        Ok(CallToolResult::text(format!("{:?}", response)))
    }
}

impl Default for GdbMcpServer {
    fn default() -> Self {
        Self::new()
    }
}

//! GDB Client Implementation
//!
//! Manages GDB process lifecycle and communication via Machine Interface (MI).

use crate::gdb::parser::{
    parse_breakpoint, parse_breakpoint_list, parse_frame, parse_memory_content,
    parse_register_names, parse_register_values, parse_stack_frames, parse_thread_ids,
    parse_variable, parse_variable_children, parse_watchpoint, MiParser,
};
use crate::gdb::types::*;
use crate::gdb::types::WatchpointType;
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tracing::{debug, error, info, warn};

/// GDB Client for managing debugging sessions
pub struct GdbClient {
    /// GDB process
    process: Option<Child>,
    /// Standard input to GDB
    stdin: Option<ChildStdin>,
    /// Token counter for MI commands
    token_counter: AtomicU64,
    /// Configuration
    config: GdbConfig,
    /// Pending responses by token
    pending_responses: Arc<Mutex<HashMap<u64, Sender<MiOutputRecord>>>>,
    /// Event receiver
    event_rx: Option<Receiver<GdbEvent>>,
    /// Event sender (cloned for background thread)
    event_tx: Sender<GdbEvent>,
    /// Output reader thread handle
    reader_handle: Option<JoinHandle<()>>,
    /// Session state
    state: Arc<Mutex<GdbSessionState>>,
}

impl GdbClient {
    /// Create a new GDB client with the given configuration
    pub fn new(config: GdbConfig) -> Self {
        let (event_tx, event_rx) = mpsc::channel();
        Self {
            process: None,
            stdin: None,
            token_counter: AtomicU64::new(1),
            config,
            pending_responses: Arc::new(Mutex::new(HashMap::new())),
            event_rx: Some(event_rx),
            event_tx,
            reader_handle: None,
            state: Arc::new(Mutex::new(GdbSessionState::default())),
        }
    }

    /// Start the GDB process
    pub fn start(&mut self) -> Result<()> {
        if self.process.is_some() {
            return Err(anyhow!("GDB process already running"));
        }

        info!("Starting GDB: {}", self.config.gdb_path);

        let mut cmd = Command::new(&self.config.gdb_path);
        cmd.args(&self.config.gdb_args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut process = cmd.spawn()?;
        
        let stdin = process.stdin.take().ok_or_else(|| anyhow!("Failed to get stdin"))?;
        let stdout = process.stdout.take().ok_or_else(|| anyhow!("Failed to get stdout"))?;
        let stderr = process.stderr.take().ok_or_else(|| anyhow!("Failed to get stderr"))?;

        self.stdin = Some(stdin);
        self.process = Some(process);

        // Start output reader thread
        let pending = Arc::clone(&self.pending_responses);
        let event_tx = self.event_tx.clone();
        let state = Arc::clone(&self.state);
        
        let stdout_reader = BufReader::new(stdout);
        let reader_handle = thread::spawn(move || {
            Self::read_output_loop(stdout_reader, pending, event_tx, state);
        });
        self.reader_handle = Some(reader_handle);

        // Start stderr reader thread
        let event_tx_stderr = self.event_tx.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines() {
                if let Ok(line) = line {
                    debug!("GDB stderr: {}", line);
                    let _ = event_tx_stderr.send(GdbEvent::Output {
                        channel: OutputChannel::Log,
                        content: line,
                    });
                }
            }
        });

        // Wait for initial (gdb) prompt
        thread::sleep(Duration::from_millis(500));

        // Initialize GDB
        self.initialize()?;

        {
            let mut state = self.state.lock().unwrap();
            state.connected = true;
        }

        info!("GDB started successfully");
        Ok(())
    }

    /// Initialize GDB with necessary settings
    fn initialize(&mut self) -> Result<()> {
        // Enable async mode
        self.send_command("gdb-set mi-async on")?;
        
        // Set pagination off
        self.send_command("gdb-set pagination off")?;
        
        // Set confirmations off
        self.send_command("gdb-set confirm off")?;
        
        Ok(())
    }

    /// Read output loop (runs in background thread)
    fn read_output_loop(
        reader: BufReader<ChildStdout>,
        pending: Arc<Mutex<HashMap<u64, Sender<MiOutputRecord>>>>,
        event_tx: Sender<GdbEvent>,
        state: Arc<Mutex<GdbSessionState>>,
    ) {
        let parser = crate::gdb::parser::MiParser::new();
        
        for line in reader.lines() {
            match line {
                Ok(line) => {
                    debug!("GDB output: {}", line);
                    
                    match parser.parse_line(&line) {
                        Ok(Some(record)) => {
                            // Check if this is a response to a pending command
                            if let MiOutputRecord::Result { token, .. } = &record {
                                if let Some(tok) = token {
                                    let pending_map = pending.lock().unwrap();
                                    if let Some(tx) = pending_map.get(tok) {
                                        let _ = tx.send(record);
                                        continue;
                                    }
                                }
                            }
                            
                            // Process async records and notifications
                            Self::handle_async_record(&record, &event_tx, &state);
                        }
                        Ok(None) => {
                            // Empty line or (gdb) prompt - ignore
                        }
                        Err(e) => {
                            warn!("Failed to parse line: {} - {}", line, e);
                        }
                    }
                }
                Err(e) => {
                    error!("Error reading GDB output: {}", e);
                    break;
                }
            }
        }
        
        info!("GDB output reader stopped");
    }

    /// Handle async records and notifications
    fn handle_async_record(
        record: &MiOutputRecord,
        event_tx: &Sender<GdbEvent>,
        state: &Arc<Mutex<GdbSessionState>>,
    ) {
        match record {
            MiOutputRecord::Async { class, results, .. } => {
                match class {
                    AsyncClass::Stopped => {
                        let reason = results.iter()
                            .find(|r| r.variable == "reason")
                            .and_then(|r| {
                                if let MiValue::String(s) = &r.value {
                                    Some(StopReason::from(s.clone()))
                                } else {
                                    None
                                }
                            })
                            .unwrap_or(StopReason::Unknown("unknown".to_string()));
                        
                        let frame = parse_frame(results);
                        let thread_id = results.iter()
                            .find(|r| r.variable == "thread-id")
                            .and_then(|r| {
                                if let MiValue::String(s) = &r.value {
                                    Some(s.clone())
                                } else {
                                    None
                                }
                            });

                        {
                            let mut state = state.lock().unwrap();
                            state.running = false;
                            state.current_thread = thread_id.clone();
                        }

                        let _ = event_tx.send(GdbEvent::Stopped {
                            reason,
                            frame,
                            thread_id,
                        });
                    }
                    AsyncClass::Running => {
                        let thread_id = results.iter()
                            .find(|r| r.variable == "thread-id")
                            .and_then(|r| {
                                if let MiValue::String(s) = &r.value {
                                    Some(s.clone())
                                } else {
                                    None
                                }
                            });

                        {
                            let mut state = state.lock().unwrap();
                            state.running = true;
                        }

                        let _ = event_tx.send(GdbEvent::Running { thread_id });
                    }
                }
            }
            MiOutputRecord::Notification { class, results } => {
                match class {
                    NotificationClass::BreakpointCreated => {
                        if let Some(bp) = parse_breakpoint(results) {
                            let _ = event_tx.send(GdbEvent::BreakpointCreated { breakpoint: bp });
                        }
                    }
                    NotificationClass::BreakpointModified => {
                        if let Some(bp) = parse_breakpoint(results) {
                            let _ = event_tx.send(GdbEvent::BreakpointModified { breakpoint: bp });
                        }
                    }
                    NotificationClass::BreakpointDeleted => {
                        let number = results.iter()
                            .find(|r| r.variable == "number")
                            .and_then(|r| {
                                if let MiValue::String(s) = &r.value {
                                    Some(s.clone())
                                } else {
                                    None
                                }
                            });
                        if let Some(num) = number {
                            let _ = event_tx.send(GdbEvent::BreakpointDeleted { number: num });
                        }
                    }
                    NotificationClass::ThreadCreated => {
                        let id = results.iter()
                            .find(|r| r.variable == "id")
                            .and_then(|r| {
                                if let MiValue::String(s) = &r.value {
                                    Some(s.clone())
                                } else {
                                    None
                                }
                            });
                        let group_id = results.iter()
                            .find(|r| r.variable == "group-id")
                            .and_then(|r| {
                                if let MiValue::String(s) = &r.value {
                                    Some(s.clone())
                                } else {
                                    None
                                }
                            });
                        if let (Some(id), Some(group_id)) = (id, group_id) {
                            let _ = event_tx.send(GdbEvent::ThreadCreated { id, group_id });
                        }
                    }
                    NotificationClass::ThreadExited => {
                        let id = results.iter()
                            .find(|r| r.variable == "id")
                            .and_then(|r| {
                                if let MiValue::String(s) = &r.value {
                                    Some(s.clone())
                                } else {
                                    None
                                }
                            });
                        let group_id = results.iter()
                            .find(|r| r.variable == "group-id")
                            .and_then(|r| {
                                if let MiValue::String(s) = &r.value {
                                    Some(s.clone())
                                } else {
                                    None
                                }
                            });
                        if let (Some(id), Some(group_id)) = (id, group_id) {
                            let _ = event_tx.send(GdbEvent::ThreadExited { id, group_id });
                        }
                    }
                    NotificationClass::ThreadSelected => {
                        let id = results.iter()
                            .find(|r| r.variable == "id")
                            .and_then(|r| {
                                if let MiValue::String(s) = &r.value {
                                    Some(s.clone())
                                } else {
                                    None
                                }
                            });
                        if let Some(id) = id {
                            let mut state = state.lock().unwrap();
                            state.current_thread = Some(id.clone());
                            let _ = event_tx.send(GdbEvent::ThreadSelected { id });
                        }
                    }
                    _ => {}
                }
            }
            MiOutputRecord::Console(content) => {
                let _ = event_tx.send(GdbEvent::Output {
                    channel: OutputChannel::Console,
                    content: content.clone(),
                });
            }
            MiOutputRecord::Target(content) => {
                let _ = event_tx.send(GdbEvent::Output {
                    channel: OutputChannel::Target,
                    content: content.clone(),
                });
            }
            MiOutputRecord::Log(content) => {
                let _ = event_tx.send(GdbEvent::Output {
                    channel: OutputChannel::Log,
                    content: content.clone(),
                });
            }
            _ => {}
        }
    }

    /// Send an MI command and wait for response
    pub fn send_command(&mut self, command: &str) -> Result<MiOutputRecord> {
        let stdin = self.stdin.as_mut().ok_or_else(|| anyhow!("GDB not running"))?;
        
        let token = self.token_counter.fetch_add(1, Ordering::SeqCst);
        
        // Create response channel
        let (tx, rx) = mpsc::channel();
        
        // Register pending response
        {
            let mut pending = self.pending_responses.lock().unwrap();
            pending.insert(token, tx);
        }
        
        // Send command
        let full_command = format!("{}-{}\n", token, command);
        debug!("Sending command: {}", full_command.trim());
        
        stdin.write_all(full_command.as_bytes())?;
        stdin.flush()?;
        
        // Wait for response with timeout
        let timeout = Duration::from_millis(self.config.timeout_ms);
        let response = rx.recv_timeout(timeout)
            .map_err(|_| anyhow!("Timeout waiting for GDB response"))?;
        
        // Cleanup pending
        {
            let mut pending = self.pending_responses.lock().unwrap();
            pending.remove(&token);
        }
        
        Ok(response)
    }

    /// Send a command without waiting for response (fire and forget)
    pub fn send_command_async(&mut self, command: &str) -> Result<()> {
        let stdin = self.stdin.as_mut().ok_or_else(|| anyhow!("GDB not running"))?;
        
        let token = self.token_counter.fetch_add(1, Ordering::SeqCst);
        let full_command = format!("{}-{}\n", token, command);
        
        debug!("Sending async command: {}", full_command.trim());
        
        stdin.write_all(full_command.as_bytes())?;
        stdin.flush()?;
        
        Ok(())
    }

    /// Get the event receiver
    pub fn event_receiver(&mut self) -> Option<Receiver<GdbEvent>> {
        self.event_rx.take()
    }

    /// Get current session state
    pub fn state(&self) -> GdbSessionState {
        self.state.lock().unwrap().clone()
    }

    /// Check if GDB is running
    pub fn is_running(&self) -> bool {
        self.process.is_some()
    }

    /// Stop the GDB process
    pub fn stop(&mut self) -> Result<()> {
        if let Some(mut process) = self.process.take() {
            // Try to exit GDB gracefully first
            if let Some(stdin) = self.stdin.as_mut() {
                let _ = stdin.write_all(b"-gdb-exit\n");
                let _ = stdin.flush();
            }
            
            // Wait a bit for graceful exit
            thread::sleep(Duration::from_millis(500));
            
            // Kill if still running
            let _ = process.kill();
            let _ = process.wait();
            
            self.stdin = None;
            
            {
                let mut state = self.state.lock().unwrap();
                state.connected = false;
                state.running = false;
            }
            
            info!("GDB stopped");
        }
        Ok(())
    }
}

impl Drop for GdbClient {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

/// High-level GDB operations
impl GdbClient {
    /// Load an executable file
    pub fn file_exec_and_symbols(&mut self, file: &str) -> Result<()> {
        let response = self.send_command(&format!("file-exec-and-symbols {}", file))?;
        
        match response {
            MiOutputRecord::Result { class: ResultClass::Done, .. } => {
                let mut state = self.state.lock().unwrap();
                state.executable = Some(file.to_string());
                Ok(())
            }
            MiOutputRecord::Result { class: ResultClass::Error, results, .. } => {
                let msg = results.iter()
                    .find(|r| r.variable == "msg")
                    .and_then(|r| {
                        if let MiValue::String(s) = &r.value {
                            Some(s.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| "Unknown error".to_string());
                Err(anyhow!("Failed to load file: {}", msg))
            }
            _ => Err(anyhow!("Unexpected response")),
        }
    }

    /// Connect to a remote target
    pub fn target_connect_remote(&mut self, target: &str) -> Result<()> {
        let response = self.send_command(&format!("target-select remote {}", target))?;
        
        match response {
            MiOutputRecord::Result { class: ResultClass::Connected, .. } |
            MiOutputRecord::Result { class: ResultClass::Done, .. } => {
                let mut state = self.state.lock().unwrap();
                state.target_remote = true;
                Ok(())
            }
            MiOutputRecord::Result { class: ResultClass::Error, results, .. } => {
                let msg = results.iter()
                    .find(|r| r.variable == "msg")
                    .and_then(|r| {
                        if let MiValue::String(s) = &r.value {
                            Some(s.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| "Unknown error".to_string());
                Err(anyhow!("Failed to connect to target: {}", msg))
            }
            _ => Err(anyhow!("Unexpected response")),
        }
    }

    /// Connect to extended remote target
    pub fn target_connect_extended_remote(&mut self, target: &str) -> Result<()> {
        let response = self.send_command(&format!("target-select extended-remote {}", target))?;
        
        match response {
            MiOutputRecord::Result { class: ResultClass::Connected, .. } |
            MiOutputRecord::Result { class: ResultClass::Done, .. } => {
                let mut state = self.state.lock().unwrap();
                state.target_remote = true;
                Ok(())
            }
            MiOutputRecord::Result { class: ResultClass::Error, results, .. } => {
                let msg = results.iter()
                    .find(|r| r.variable == "msg")
                    .and_then(|r| {
                        if let MiValue::String(s) = &r.value {
                            Some(s.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| "Unknown error".to_string());
                Err(anyhow!("Failed to connect to extended-remote target: {}", msg))
            }
            _ => Err(anyhow!("Unexpected response")),
        }
    }

    /// Disconnect from remote target
    pub fn target_disconnect(&mut self) -> Result<()> {
        let response = self.send_command("target-disconnect")?;
        
        match response {
            MiOutputRecord::Result { class: ResultClass::Done, .. } => {
                let mut state = self.state.lock().unwrap();
                state.target_remote = false;
                Ok(())
            }
            MiOutputRecord::Result { class: ResultClass::Error, results, .. } => {
                let msg = results.iter()
                    .find(|r| r.variable == "msg")
                    .and_then(|r| {
                        if let MiValue::String(s) = &r.value {
                            Some(s.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| "Unknown error".to_string());
                Err(anyhow!("Failed to disconnect: {}", msg))
            }
            _ => Err(anyhow!("Unexpected response")),
        }
    }

    /// Set architecture
    pub fn set_architecture(&mut self, arch: &str) -> Result<()> {
        let response = self.send_command(&format!("gdb-set architecture {}", arch))?;
        
        match response {
            MiOutputRecord::Result { class: ResultClass::Done, .. } => {
                let mut state = self.state.lock().unwrap();
                state.architecture = Some(arch.to_string());
                Ok(())
            }
            MiOutputRecord::Result { class: ResultClass::Error, results, .. } => {
                let msg = results.iter()
                    .find(|r| r.variable == "msg")
                    .and_then(|r| {
                        if let MiValue::String(s) = &r.value {
                            Some(s.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| "Unknown error".to_string());
                Err(anyhow!("Failed to set architecture: {}", msg))
            }
            _ => Err(anyhow!("Unexpected response")),
        }
    }

    /// Insert a breakpoint
    pub fn break_insert(&mut self, location: &str, temporary: bool, condition: Option<&str>) -> Result<Breakpoint> {
        let mut cmd = String::from("break-insert");
        if temporary {
            cmd.push_str(" -t");
        }
        if let Some(cond) = condition {
            cmd.push_str(&format!(" -c \"{}\"", cond));
        }
        cmd.push_str(&format!(" {}", location));
        
        let response = self.send_command(&cmd)?;
        
        match response {
            MiOutputRecord::Result { class: ResultClass::Done, results, .. } => {
                parse_breakpoint(&results)
                    .ok_or_else(|| anyhow!("Failed to parse breakpoint response"))
            }
            MiOutputRecord::Result { class: ResultClass::Error, results, .. } => {
                let msg = results.iter()
                    .find(|r| r.variable == "msg")
                    .and_then(|r| {
                        if let MiValue::String(s) = &r.value {
                            Some(s.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| "Unknown error".to_string());
                Err(anyhow!("Failed to insert breakpoint: {}", msg))
            }
            _ => Err(anyhow!("Unexpected response")),
        }
    }

    /// Delete a breakpoint
    pub fn break_delete(&mut self, number: &str) -> Result<()> {
        let response = self.send_command(&format!("break-delete {}", number))?;
        
        match response {
            MiOutputRecord::Result { class: ResultClass::Done, .. } => Ok(()),
            MiOutputRecord::Result { class: ResultClass::Error, results, .. } => {
                let msg = results.iter()
                    .find(|r| r.variable == "msg")
                    .and_then(|r| {
                        if let MiValue::String(s) = &r.value {
                            Some(s.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| "Unknown error".to_string());
                Err(anyhow!("Failed to delete breakpoint: {}", msg))
            }
            _ => Err(anyhow!("Unexpected response")),
        }
    }

    /// Enable a breakpoint
    pub fn break_enable(&mut self, number: &str) -> Result<()> {
        let response = self.send_command(&format!("break-enable {}", number))?;
        
        match response {
            MiOutputRecord::Result { class: ResultClass::Done, .. } => Ok(()),
            _ => Err(anyhow!("Failed to enable breakpoint")),
        }
    }

    /// Disable a breakpoint
    pub fn break_disable(&mut self, number: &str) -> Result<()> {
        let response = self.send_command(&format!("break-disable {}", number))?;
        
        match response {
            MiOutputRecord::Result { class: ResultClass::Done, .. } => Ok(()),
            _ => Err(anyhow!("Failed to disable breakpoint")),
        }
    }

    /// List breakpoints
    pub fn break_list(&mut self) -> Result<Vec<Breakpoint>> {
        let response = self.send_command("break-list")?;
        
        match response {
            MiOutputRecord::Result { class: ResultClass::Done, results, .. } => {
                Ok(parse_breakpoint_list(&results))
            }
            _ => Ok(Vec::new()),
        }
    }

    /// Insert a watchpoint
    pub fn watch_insert(&mut self, wp_type: WatchpointType, location: &str) -> Result<Watchpoint> {
        let type_arg = match wp_type {
            WatchpointType::Write => "",
            WatchpointType::Read => "-r",
            WatchpointType::Access => "-a",
        };
        
        let cmd = if type_arg.is_empty() {
            format!("break-watch {}", location)
        } else {
            format!("break-watch {} {}", type_arg, location)
        };
        
        let response = self.send_command(&cmd)?;
        
        match response {
            MiOutputRecord::Result { class: ResultClass::Done, results, .. } => {
                parse_watchpoint(&results, wp_type)
                    .ok_or_else(|| anyhow!("Failed to parse watchpoint response"))
            }
            MiOutputRecord::Result { class: ResultClass::Error, results, .. } => {
                let msg = results.iter()
                    .find(|r| r.variable == "msg")
                    .and_then(|r| MiParser::extract_string(&r.value))
                    .unwrap_or_else(|| "Unknown error".to_string());
                Err(anyhow!("Failed to insert watchpoint: {}", msg))
            }
            _ => Err(anyhow!("Unexpected response")),
        }
    }

    /// Start execution
    pub fn exec_run(&mut self) -> Result<()> {
        let response = self.send_command("exec-run")?;
        
        match response {
            MiOutputRecord::Result { class: ResultClass::Running, .. } => {
                let mut state = self.state.lock().unwrap();
                state.running = true;
                Ok(())
            }
            MiOutputRecord::Result { class: ResultClass::Error, results, .. } => {
                let msg = results.iter()
                    .find(|r| r.variable == "msg")
                    .and_then(|r| {
                        if let MiValue::String(s) = &r.value {
                            Some(s.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| "Unknown error".to_string());
                Err(anyhow!("Failed to run: {}", msg))
            }
            _ => Err(anyhow!("Unexpected response")),
        }
    }

    /// Continue execution
    pub fn exec_continue(&mut self) -> Result<()> {
        let response = self.send_command("exec-continue")?;
        
        match response {
            MiOutputRecord::Result { class: ResultClass::Running, .. } => {
                {
                    let mut state = self.state.lock().unwrap();
                    state.running = true;
                }
                self.wait_for_stop(60000)?;
                Ok(())
            }
            MiOutputRecord::Result { class: ResultClass::Error, results, .. } => {
                let msg = results.iter()
                    .find(|r| r.variable == "msg")
                    .and_then(|r| MiParser::extract_string(&r.value))
                    .unwrap_or_else(|| "Unknown error".to_string());
                Err(anyhow!("Failed to continue: {}", msg))
            }
            _ => Err(anyhow!("Unexpected response")),
        }
    }

    /// Step over
    pub fn exec_next(&mut self) -> Result<()> {
        let response = self.send_command("exec-next")?;
        
        match response {
            MiOutputRecord::Result { class: ResultClass::Running, .. } => {
                self.wait_for_stop(5000)?;
                Ok(())
            }
            MiOutputRecord::Result { class: ResultClass::Done, .. } => Ok(()),
            MiOutputRecord::Result { class: ResultClass::Error, results, .. } => {
                let msg = results.iter()
                    .find(|r| r.variable == "msg")
                    .and_then(|r| MiParser::extract_string(&r.value))
                    .unwrap_or_else(|| "Unknown error".to_string());
                Err(anyhow!("Failed to step: {}", msg))
            }
            other => {
                debug!("Unexpected step response: {:?}", other);
                Err(anyhow!("Failed to step: unexpected response"))
            }
        }
    }

    /// Step into
    pub fn exec_step(&mut self) -> Result<()> {
        let response = self.send_command("exec-step")?;
        
        match response {
            MiOutputRecord::Result { class: ResultClass::Running, .. } => {
                self.wait_for_stop(5000)?;
                Ok(())
            }
            MiOutputRecord::Result { class: ResultClass::Done, .. } => Ok(()),
            MiOutputRecord::Result { class: ResultClass::Error, results, .. } => {
                let msg = results.iter()
                    .find(|r| r.variable == "msg")
                    .and_then(|r| MiParser::extract_string(&r.value))
                    .unwrap_or_else(|| "Unknown error".to_string());
                Err(anyhow!("Failed to step: {}", msg))
            }
            other => {
                debug!("Unexpected step response: {:?}", other);
                Err(anyhow!("Failed to step: unexpected response"))
            }
        }
    }

    /// Step one instruction (assembly level)
    pub fn exec_step_instruction(&mut self) -> Result<()> {
        let response = self.send_command("exec-step-instruction")?;
        
        match response {
            MiOutputRecord::Result { class: ResultClass::Running, .. } => {
                self.wait_for_stop(5000)?;
                Ok(())
            }
            MiOutputRecord::Result { class: ResultClass::Done, .. } => Ok(()),
            MiOutputRecord::Result { class: ResultClass::Error, results, .. } => {
                let msg = results.iter()
                    .find(|r| r.variable == "msg")
                    .and_then(|r| MiParser::extract_string(&r.value))
                    .unwrap_or_else(|| "Unknown error".to_string());
                Err(anyhow!("Failed to step instruction: {}", msg))
            }
            other => {
                debug!("Unexpected step response: {:?}", other);
                Err(anyhow!("Failed to step instruction: unexpected response"))
            }
        }
    }

    /// Next one instruction (assembly level)
    pub fn exec_next_instruction(&mut self) -> Result<()> {
        let response = self.send_command("exec-next-instruction")?;
        
        match response {
            MiOutputRecord::Result { class: ResultClass::Running, .. } => {
                self.wait_for_stop(5000)?;
                Ok(())
            }
            MiOutputRecord::Result { class: ResultClass::Done, .. } => Ok(()),
            MiOutputRecord::Result { class: ResultClass::Error, results, .. } => {
                let msg = results.iter()
                    .find(|r| r.variable == "msg")
                    .and_then(|r| MiParser::extract_string(&r.value))
                    .unwrap_or_else(|| "Unknown error".to_string());
                Err(anyhow!("Failed to next instruction: {}", msg))
            }
            other => {
                debug!("Unexpected next response: {:?}", other);
                Err(anyhow!("Failed to next instruction: unexpected response"))
            }
        }
    }

    /// Wait for the target to stop
    fn wait_for_stop(&self, timeout_ms: u64) -> Result<()> {
        let start = std::time::Instant::now();
        loop {
            let state = self.state.lock().unwrap();
            if !state.running {
                return Ok(());
            }
            drop(state);
            
            if start.elapsed().as_millis() as u64 > timeout_ms {
                return Err(anyhow!("Timeout waiting for target to stop"));
            }
            
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }

    /// Step out
    pub fn exec_finish(&mut self) -> Result<()> {
        let response = self.send_command("exec-finish")?;
        
        match response {
            MiOutputRecord::Result { class: ResultClass::Running, .. } => Ok(()),
            _ => Err(anyhow!("Failed to finish")),
        }
    }

    /// Interrupt execution
    pub fn exec_interrupt(&mut self) -> Result<()> {
        let response = self.send_command("exec-interrupt")?;
        
        match response {
            MiOutputRecord::Result { class: ResultClass::Done, .. } => {
                let mut state = self.state.lock().unwrap();
                state.running = false;
                Ok(())
            }
            _ => Err(anyhow!("Failed to interrupt")),
        }
    }

    /// Get stack trace
    pub fn stack_list_frames(&mut self) -> Result<Vec<Frame>> {
        let response = self.send_command("stack-list-frames")?;
        
        match response {
            MiOutputRecord::Result { class: ResultClass::Done, results, .. } => {
                Ok(parse_stack_frames(&results))
            }
            _ => Ok(Vec::new()),
        }
    }

    /// Get current frame
    pub fn stack_info_frame(&mut self) -> Result<Option<Frame>> {
        let response = self.send_command("stack-info-frame")?;
        
        match response {
            MiOutputRecord::Result { class: ResultClass::Done, results, .. } => {
                Ok(parse_frame(&results))
            }
            _ => Ok(None),
        }
    }

    /// Select frame
    pub fn stack_select_frame(&mut self, level: u64) -> Result<()> {
        let response = self.send_command(&format!("stack-select-frame {}", level))?;
        
        match response {
            MiOutputRecord::Result { class: ResultClass::Done, .. } => {
                let mut state = self.state.lock().unwrap();
                state.current_frame = Some(level);
                Ok(())
            }
            _ => Err(anyhow!("Failed to select frame")),
        }
    }

    /// List threads
    pub fn thread_list_ids(&mut self) -> Result<Vec<String>> {
        let response = self.send_command("thread-list-ids")?;
        
        match response {
            MiOutputRecord::Result { class: ResultClass::Done, results, .. } => {
                Ok(parse_thread_ids(&results))
            }
            _ => Ok(Vec::new()),
        }
    }

    /// Select thread
    pub fn thread_select(&mut self, id: &str) -> Result<()> {
        let response = self.send_command(&format!("thread-select {}", id))?;
        
        match response {
            MiOutputRecord::Result { class: ResultClass::Done, .. } => {
                let mut state = self.state.lock().unwrap();
                state.current_thread = Some(id.to_string());
                Ok(())
            }
            _ => Err(anyhow!("Failed to select thread")),
        }
    }

    /// Read memory
    pub fn data_read_memory(&mut self, addr: &str, count: u64) -> Result<MemoryContent> {
        let response = self.send_command(&format!("data-read-memory-bytes {} {}", addr, count))?;
        
        match response {
            MiOutputRecord::Result { class: ResultClass::Done, results, .. } => {
                parse_memory_content(&results)
                    .ok_or_else(|| anyhow!("Failed to parse memory content"))
            }
            _ => Err(anyhow!("Failed to read memory")),
        }
    }

    /// Evaluate expression
    pub fn data_evaluate_expression(&mut self, expr: &str) -> Result<String> {
        let response = self.send_command(&format!("data-evaluate-expression \"{}\"", expr))?;
        
        match response {
            MiOutputRecord::Result { class: ResultClass::Done, results, .. } => {
                let value = results.iter()
                    .find(|r| r.variable == "value")
                    .and_then(|r| {
                        if let MiValue::String(s) = &r.value {
                            Some(s.clone())
                        } else {
                            None
                        }
                    });
                value.ok_or_else(|| anyhow!("No value in response"))
            }
            _ => Err(anyhow!("Failed to evaluate expression")),
        }
    }

    /// List registers
    pub fn data_list_register_names(&mut self) -> Result<Vec<String>> {
        let response = self.send_command("data-list-register-names")?;
        
        match response {
            MiOutputRecord::Result { class: ResultClass::Done, results, .. } => {
                Ok(parse_register_names(&results))
            }
            _ => Ok(Vec::new()),
        }
    }

    /// Get register values
    pub fn data_list_register_values(&mut self) -> Result<Vec<Register>> {
        let response = self.send_command("data-list-register-values --skip-unavailable")?;
        
        match response {
            MiOutputRecord::Result { class: ResultClass::Done, results, .. } => {
                Ok(parse_register_values(&results))
            }
            _ => Ok(Vec::new()),
        }
    }

    /// Create variable object
    pub fn var_create(&mut self, name: &str, frame_addr: Option<&str>) -> Result<Variable> {
        let mut cmd = format!("var-create - * \"{}\"", name);
        if let Some(addr) = frame_addr {
            cmd = format!("var-create --frame {} - * \"{}\"", addr, name);
        }
        
        let response = self.send_command(&cmd)?;
        
        match response {
            MiOutputRecord::Result { class: ResultClass::Done, results, .. } => {
                parse_variable(&results, name)
                    .ok_or_else(|| anyhow!("Failed to parse variable"))
            }
            MiOutputRecord::Result { class: ResultClass::Error, results, .. } => {
                let msg = results.iter()
                    .find(|r| r.variable == "msg")
                    .and_then(|r| MiParser::extract_string(&r.value))
                    .unwrap_or_else(|| "Unknown error".to_string());
                Err(anyhow!("Failed to create variable: {}", msg))
            }
            _ => Err(anyhow!("Unexpected response")),
        }
    }

    /// Delete variable object
    pub fn var_delete(&mut self, name: &str) -> Result<()> {
        let response = self.send_command(&format!("var-delete {}", name))?;
        
        match response {
            MiOutputRecord::Result { class: ResultClass::Done, .. } => Ok(()),
            _ => Err(anyhow!("Failed to delete variable")),
        }
    }

    /// Evaluate variable
    pub fn var_evaluate_expression(&mut self, name: &str) -> Result<String> {
        let response = self.send_command(&format!("var-evaluate-expression {}", name))?;
        
        match response {
            MiOutputRecord::Result { class: ResultClass::Done, results, .. } => {
                let value = results.iter()
                    .find(|r| r.variable == "value")
                    .and_then(|r| {
                        if let MiValue::String(s) = &r.value {
                            Some(s.clone())
                        } else {
                            None
                        }
                    });
                value.ok_or_else(|| anyhow!("No value in response"))
            }
            _ => Err(anyhow!("Failed to evaluate variable")),
        }
    }

    /// List children of a variable
    pub fn var_list_children(&mut self, name: &str) -> Result<Vec<Variable>> {
        let response = self.send_command(&format!("var-list-children --all-values {}", name))?;
        
        match response {
            MiOutputRecord::Result { class: ResultClass::Done, results, .. } => {
                Ok(parse_variable_children(&results))
            }
            _ => Ok(Vec::new()),
        }
    }
}

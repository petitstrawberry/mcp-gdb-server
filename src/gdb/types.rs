//! GDB Machine Interface (MI) Type Definitions

use serde::{Deserialize, Serialize};

/// GDB/MI result class types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResultClass {
    Done,
    Running,
    Connected,
    Error,
    Exit,
}

/// GDB/MI async class types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AsyncClass {
    Stopped,
    Running,
}

/// GDB/MI notification types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NotificationClass {
    BreakpointCreated,
    BreakpointModified,
    BreakpointDeleted,
    ThreadGroupAdded,
    ThreadGroupStarted,
    ThreadGroupExited,
    ThreadCreated,
    ThreadSelected,
    ThreadExited,
    LibraryLoaded,
    LibraryUnloaded,
    CmdParamChanged,
    MemoryChanged,
    ParamChanged,
}

/// Stop reason types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StopReason {
    BreakpointHit,
    WatchpointTrigger,
    ReadWatchpointTrigger,
    AccessWatchpointTrigger,
    FunctionFinished,
    LocationReached,
    WatchpointScope,
    EndSteppingRange,
    ExitedSignalled,
    Exited,
    ExitedNormally,
    SignalReceived,
    SolibEvent,
    Fork,
    Vfork,
    SyscallEntry,
    SyscallReturn,
    Unknown(String),
}

impl From<String> for StopReason {
    fn from(s: String) -> Self {
        match s.as_str() {
            "breakpoint-hit" => StopReason::BreakpointHit,
            "watchpoint-trigger" => StopReason::WatchpointTrigger,
            "read-watchpoint-trigger" => StopReason::ReadWatchpointTrigger,
            "access-watchpoint-trigger" => StopReason::AccessWatchpointTrigger,
            "function-finished" => StopReason::FunctionFinished,
            "location-reached" => StopReason::LocationReached,
            "watchpoint-scope" => StopReason::WatchpointScope,
            "end-stepping-range" => StopReason::EndSteppingRange,
            "exited-signalled" => StopReason::ExitedSignalled,
            "exited" => StopReason::Exited,
            "exited-normally" => StopReason::ExitedNormally,
            "signal-received" => StopReason::SignalReceived,
            "solib-event" => StopReason::SolibEvent,
            "fork" => StopReason::Fork,
            "vfork" => StopReason::Vfork,
            "syscall-entry" => StopReason::SyscallEntry,
            "syscall-return" => StopReason::SyscallReturn,
            _ => StopReason::Unknown(s),
        }
    }
}

/// GDB/MI value types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MiValue {
    String(String),
    List(Vec<MiValue>),
    Tuple(MiTuple),
    None,
}

pub type MiTuple = std::collections::HashMap<String, MiValue>;

/// GDB/MI output record
#[derive(Debug, Clone)]
pub enum MiOutputRecord {
    Result {
        token: Option<u64>,
        class: ResultClass,
        results: Vec<MiResult>,
    },
    Async {
        token: Option<u64>,
        class: AsyncClass,
        results: Vec<MiResult>,
    },
    Notification {
        class: NotificationClass,
        results: Vec<MiResult>,
    },
    Console(String),
    Target(String),
    Log(String),
}

/// GDB/MI result (variable=value pair)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiResult {
    pub variable: String,
    pub value: MiValue,
}

/// Breakpoint information
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Breakpoint {
    pub number: String,
    #[serde(rename = "type")]
    pub breakpoint_type: String,
    pub disposition: String,
    pub enabled: bool,
    #[serde(default)]
    pub addr: Option<String>,
    #[serde(default)]
    pub func: Option<String>,
    #[serde(default)]
    pub file: Option<String>,
    #[serde(default)]
    pub fullname: Option<String>,
    #[serde(default)]
    pub line: Option<u64>,
    #[serde(default)]
    pub thread_groups: Option<Vec<String>>,
    #[serde(default)]
    pub times: u64,
    #[serde(default)]
    pub original_location: Option<String>,
    #[serde(default)]
    pub condition: Option<String>,
    #[serde(default)]
    pub ignore_count: Option<u64>,
}

/// Watchpoint type
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WatchpointType {
    Write,
    Read,
    Access,
}

/// Watchpoint information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Watchpoint {
    pub number: String,
    #[serde(rename = "type")]
    pub watchpoint_type: WatchpointType,
    pub enabled: bool,
    pub addr: String,
    #[serde(default)]
    pub exp: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub old_value: Option<String>,
    #[serde(default)]
    pub times: u64,
    #[serde(default)]
    pub condition: Option<String>,
}

/// Frame information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Frame {
    pub level: u64,
    pub addr: String,
    #[serde(default)]
    pub func: Option<String>,
    #[serde(default)]
    pub file: Option<String>,
    #[serde(default)]
    pub fullname: Option<String>,
    #[serde(default)]
    pub line: Option<u64>,
    #[serde(default)]
    pub arch: Option<String>,
}

/// Thread information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thread {
    pub id: String,
    pub target_id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub frame: Option<Frame>,
    pub state: ThreadState,
    #[serde(default)]
    pub core: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThreadState {
    Stopped,
    Running,
}

/// Variable information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Variable {
    pub name: String,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub var_type: Option<String>,
    #[serde(default)]
    pub attributes: Option<Vec<String>>,
    #[serde(default)]
    pub children: Option<Vec<Variable>>,
}

/// Register information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Register {
    pub number: u64,
    pub name: String,
    pub value: String,
}

/// Memory content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryContent {
    pub addr: String,
    pub data: Vec<String>,
}

/// Stack arguments
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackArgs {
    pub frame: Frame,
    pub args: Vec<Argument>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Argument {
    pub name: String,
    #[serde(default)]
    pub value: Option<String>,
}

/// GDB session state
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GdbSessionState {
    pub connected: bool,
    pub running: bool,
    pub target_remote: bool,
    pub architecture: Option<String>,
    pub executable: Option<String>,
    pub current_thread: Option<String>,
    pub current_frame: Option<u64>,
}

/// GDB event types
#[derive(Debug, Clone)]
pub enum GdbEvent {
    Stopped {
        reason: StopReason,
        frame: Option<Frame>,
        thread_id: Option<String>,
    },
    Running {
        thread_id: Option<String>,
    },
    BreakpointCreated {
        breakpoint: Breakpoint,
    },
    BreakpointModified {
        breakpoint: Breakpoint,
    },
    BreakpointDeleted {
        number: String,
    },
    ThreadCreated {
        id: String,
        group_id: String,
    },
    ThreadExited {
        id: String,
        group_id: String,
    },
    ThreadSelected {
        id: String,
    },
    Error {
        message: String,
    },
    Output {
        channel: OutputChannel,
        content: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum OutputChannel {
    Console,
    Target,
    Log,
}

/// GDB configuration
#[derive(Debug, Clone)]
pub struct GdbConfig {
    pub gdb_path: String,
    pub gdb_args: Vec<String>,
    pub timeout_ms: u64,
    pub architecture: Option<String>,
}

impl Default for GdbConfig {
    fn default() -> Self {
        Self {
            gdb_path: "gdb-multiarch".to_string(),
            gdb_args: vec!["--interpreter=mi2".to_string()],
            timeout_ms: 30000,
            architecture: None,
        }
    }
}

/// Remote target configuration
#[derive(Debug, Clone)]
pub enum RemoteTargetConfig {
    Tcp {
        host: String,
        port: u16,
    },
    Serial {
        port: String,
        baud_rate: Option<u32>,
    },
}

impl RemoteTargetConfig {
    pub fn to_target_string(&self) -> String {
        match self {
            RemoteTargetConfig::Tcp { host, port } => format!("{}:{}", host, port),
            RemoteTargetConfig::Serial { port, .. } => port.clone(),
        }
    }
}

/// Error structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GdbError {
    pub code: String,
    pub message: String,
}

impl std::fmt::Display for GdbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for GdbError {}

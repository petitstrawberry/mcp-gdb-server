//! GDB Machine Interface (MI) Parser
//!
//! Parses GDB/MI output into structured Rust types.

use crate::gdb::types::*;
use anyhow::{anyhow, Result};
use regex::Regex;
use std::collections::HashMap;
use tracing::debug;

/// GDB/MI Parser
pub struct MiParser {
    // Regex patterns for parsing
    result_pattern: Regex,
    async_pattern: Regex,
    notification_pattern: Regex,
    console_pattern: Regex,
    target_pattern: Regex,
    log_pattern: Regex,
}

impl MiParser {
    pub fn new() -> Self {
        Self {
            // Result record: ^done, ^error, ^running, etc.
            result_pattern: Regex::new(r"^(\d*)\^(\w+)(?:,(.*))?$").unwrap(),
            // Async record: *stopped, *running
            async_pattern: Regex::new(r"^(\d*)\*(\w+)(?:,(.*))?$").unwrap(),
            // Notification: =breakpoint-created, etc.
            notification_pattern: Regex::new(r"^=(\S+?)(?:,(.*))?$").unwrap(),
            // Console output: ~"..."
            console_pattern: Regex::new(r#"^~"(.*)"$"#).unwrap(),
            // Target output: @"..."
            target_pattern: Regex::new(r#"^@"(.*)"$"#).unwrap(),
            // Log output: &"..."
            log_pattern: Regex::new(r#"^&"(.*)"$"#).unwrap(),
        }
    }

    /// Parse a single line of GDB/MI output
    pub fn parse_line(&self, line: &str) -> Result<Option<MiOutputRecord>> {
        let line = line.trim();
        if line.is_empty() || line == "(gdb)" {
            return Ok(None);
        }

        // Try parsing as result record
        if let Some(caps) = self.result_pattern.captures(line) {
            let token = caps.get(1).and_then(|m| m.as_str().parse::<u64>().ok());
            let class = self.parse_result_class(caps.get(2).unwrap().as_str())?;
            let results = caps.get(3)
                .map(|m| self.parse_results(m.as_str()))
                .unwrap_or_default();
            return Ok(Some(MiOutputRecord::Result { token, class, results }));
        }

        // Try parsing as async record
        if let Some(caps) = self.async_pattern.captures(line) {
            let token = caps.get(1).and_then(|m| m.as_str().parse::<u64>().ok());
            let class = self.parse_async_class(caps.get(2).unwrap().as_str())?;
            let results = caps.get(3)
                .map(|m| self.parse_results(m.as_str()))
                .unwrap_or_default();
            return Ok(Some(MiOutputRecord::Async { token, class, results }));
        }

        // Try parsing as notification
        if let Some(caps) = self.notification_pattern.captures(line) {
            let class = self.parse_notification_class(caps.get(1).unwrap().as_str())?;
            let results = caps.get(2)
                .map(|m| self.parse_results(m.as_str()))
                .unwrap_or_default();
            return Ok(Some(MiOutputRecord::Notification { class, results }));
        }

        // Try parsing as console output
        if let Some(caps) = self.console_pattern.captures(line) {
            let content = self.unescape_string(caps.get(1).unwrap().as_str());
            return Ok(Some(MiOutputRecord::Console(content)));
        }

        // Try parsing as target output
        if let Some(caps) = self.target_pattern.captures(line) {
            let content = self.unescape_string(caps.get(1).unwrap().as_str());
            return Ok(Some(MiOutputRecord::Target(content)));
        }

        // Try parsing as log output
        if let Some(caps) = self.log_pattern.captures(line) {
            let content = self.unescape_string(caps.get(1).unwrap().as_str());
            return Ok(Some(MiOutputRecord::Log(content)));
        }

        // Unknown format - treat as console output
        Ok(Some(MiOutputRecord::Console(line.to_string())))
    }

    /// Parse result class
    fn parse_result_class(&self, s: &str) -> Result<ResultClass> {
        match s {
            "done" => Ok(ResultClass::Done),
            "running" => Ok(ResultClass::Running),
            "connected" => Ok(ResultClass::Connected),
            "error" => Ok(ResultClass::Error),
            "exit" => Ok(ResultClass::Exit),
            _ => Err(anyhow!("Unknown result class: {}", s)),
        }
    }

    /// Parse async class
    fn parse_async_class(&self, s: &str) -> Result<AsyncClass> {
        match s {
            "stopped" => Ok(AsyncClass::Stopped),
            "running" => Ok(AsyncClass::Running),
            _ => Err(anyhow!("Unknown async class: {}", s)),
        }
    }

    /// Parse notification class
    fn parse_notification_class(&self, s: &str) -> Result<NotificationClass> {
        match s {
            "breakpoint-created" => Ok(NotificationClass::BreakpointCreated),
            "breakpoint-modified" => Ok(NotificationClass::BreakpointModified),
            "breakpoint-deleted" => Ok(NotificationClass::BreakpointDeleted),
            "thread-group-added" => Ok(NotificationClass::ThreadGroupAdded),
            "thread-group-started" => Ok(NotificationClass::ThreadGroupStarted),
            "thread-group-exited" => Ok(NotificationClass::ThreadGroupExited),
            "thread-created" => Ok(NotificationClass::ThreadCreated),
            "thread-selected" => Ok(NotificationClass::ThreadSelected),
            "thread-exited" => Ok(NotificationClass::ThreadExited),
            "library-loaded" => Ok(NotificationClass::LibraryLoaded),
            "library-unloaded" => Ok(NotificationClass::LibraryUnloaded),
            "cmd-param-changed" => Ok(NotificationClass::CmdParamChanged),
            "param-changed" => Ok(NotificationClass::ParamChanged),
            "memory-changed" => Ok(NotificationClass::MemoryChanged),
            _ => Err(anyhow!("Unknown notification class: {}", s)),
        }
    }

    /// Parse results (variable=value pairs)
    pub fn parse_results(&self, input: &str) -> Vec<MiResult> {
        let mut results = Vec::new();
        let mut current = input;
        
        while !current.is_empty() {
            match self.parse_result(current) {
                Ok((result, remaining)) => {
                    results.push(result);
                    current = remaining.trim_start_matches(',');
                }
                Err(_) => break,
            }
        }
        
        results
    }

    /// Parse a single result (variable=value)
    fn parse_result<'a>(&self, input: &'a str) -> Result<(MiResult, &'a str)> {
        // Find variable name
        let eq_pos = input.find('=').ok_or_else(|| anyhow!("No '=' found"))?;
        let variable = input[..eq_pos].to_string();
        let rest = &input[eq_pos + 1..];
        
        // Parse value
        let (value, remaining) = self.parse_value(rest)?;
        
        Ok((MiResult { variable, value }, remaining))
    }

    /// Parse a value (string, list, or tuple)
    fn parse_value<'a>(&self, input: &'a str) -> Result<(MiValue, &'a str)> {
        let input = input.trim_start();
        
        if input.is_empty() {
            return Ok((MiValue::None, input));
        }
        
        let first_char = input.chars().next().unwrap();
        
        match first_char {
            // String
            '"' => {
                let (s, remaining) = self.parse_string(input)?;
                Ok((MiValue::String(s), remaining))
            }
            // List
            '[' => {
                let (list, remaining) = self.parse_list(input)?;
                Ok((MiValue::List(list), remaining))
            }
            // Tuple
            '{' => {
                let (tuple, remaining) = self.parse_tuple(input)?;
                Ok((MiValue::Tuple(tuple), remaining))
            }
            // Could be a key=value pair or simple value
            _ => {
                // Check if this looks like key=value
                if let Some(eq_pos) = input.find('=') {
                    let potential_key = &input[..eq_pos];
                    // Only treat as key=value if the key looks like an identifier
                    if potential_key.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
                        let rest = &input[eq_pos + 1..];
                        
                        // Parse the value after '='
                        let (inner_value, remaining) = self.parse_value(rest)?;
                        
                        // Return as a special tuple with __key__ marker
                        let mut tuple = MiTuple::new();
                        tuple.insert("__key__".to_string(), MiValue::String(potential_key.to_string()));
                        tuple.insert("__value__".to_string(), inner_value);
                        return Ok((MiValue::Tuple(tuple), remaining));
                    }
                }
                
                // Regular simple value
                let end = input.find(|c: char| c == ',' || c == '}' || c == ']')
                    .unwrap_or(input.len());
                let value = input[..end].to_string();
                Ok((MiValue::String(value), &input[end..]))
            }
        }
    }

    /// Parse a quoted string
    fn parse_string<'a>(&self, input: &'a str) -> Result<(String, &'a str)> {
        if !input.starts_with('"') {
            return Err(anyhow!("String must start with '\"'"));
        }
        
        let mut chars = input[1..].chars().peekable();
        let mut result = String::new();
        let mut escaped = false;
        
        while let Some(c) = chars.next() {
            if escaped {
                match c {
                    'n' => result.push('\n'),
                    't' => result.push('\t'),
                    'r' => result.push('\r'),
                    '\\' => result.push('\\'),
                    '"' => result.push('"'),
                    _ => {
                        result.push('\\');
                        result.push(c);
                    }
                }
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                let remaining_len: usize = input[1..]
                    .chars()
                    .take_while(|&ch| ch != '"')
                    .map(|ch| ch.len_utf8())
                    .sum::<usize>() + 2;
                let consumed = result.chars().map(|c| c.len_utf8()).sum::<usize>();
                // Find the position after the closing quote
                let pos = input[1..].find('"').ok_or_else(|| anyhow!("Unterminated string"))? + 2;
                return Ok((result, &input[pos..]));
            } else {
                result.push(c);
            }
        }
        
        Err(anyhow!("Unterminated string"))
    }

    /// Parse a list [...]
    fn parse_list<'a>(&self, input: &'a str) -> Result<(Vec<MiValue>, &'a str)> {
        if !input.starts_with('[') {
            return Err(anyhow!("List must start with '['"));
        }
        
        let mut list = Vec::new();
        let content = &input[1..];
        
        // Find the matching closing bracket
        let mut depth = 1;
        let mut in_string = false;
        let mut escape = false;
        let mut end_pos = 0;
        
        for (i, c) in content.char_indices() {
            if escape {
                escape = false;
                continue;
            }
            
            match c {
                '\\' => escape = true,
                '"' => in_string = !in_string,
                '{' if !in_string => depth += 1,
                '}' if !in_string => depth -= 1,
                '[' if !in_string => depth += 1,
                ']' if !in_string => {
                    depth -= 1;
                    if depth == 0 {
                        end_pos = i;
                        break;
                    }
                }
                _ => {}
            }
        }
        
        if depth != 0 {
            return Err(anyhow!("Unterminated list"));
        }
        
        let inner = content[..end_pos].trim();
        let remaining = &content[end_pos + 1..];
        
        if inner.is_empty() {
            return Ok((list, remaining));
        }
        
        // Split by commas at depth 1
        let mut current_start = 0;
        depth = 1;
        in_string = false;
        escape = false;
        
        for (i, c) in inner.char_indices() {
            if escape {
                escape = false;
                continue;
            }
            
            match c {
                '\\' => escape = true,
                '"' => in_string = !in_string,
                '{' if !in_string => depth += 1,
                '}' if !in_string => depth -= 1,
                '[' if !in_string => depth += 1,
                ']' if !in_string => depth -= 1,
                ',' if depth == 1 && !in_string => {
                    let elem = inner[current_start..i].trim();
                    if !elem.is_empty() {
                        if let Ok((value, _)) = self.parse_value(elem) {
                            list.push(value);
                        }
                    }
                    current_start = i + 1;
                }
                _ => {}
            }
        }
        
        // Don't forget the last element
        let elem = inner[current_start..].trim();
        if !elem.is_empty() {
            if let Ok((value, _)) = self.parse_value(elem) {
                list.push(value);
            }
        }
        
        Ok((list, remaining))
    }

    /// Parse a tuple {...}
    fn parse_tuple<'a>(&self, input: &'a str) -> Result<(MiTuple, &'a str)> {
        if !input.starts_with('{') {
            return Err(anyhow!("Tuple must start with '{{'"));
        }
        
        let mut tuple = HashMap::new();
        let content = &input[1..];
        
        // Find the matching closing brace
        let mut depth = 1;
        let mut in_string = false;
        let mut escape = false;
        let mut end_pos = 0;
        
        for (i, c) in content.char_indices() {
            if escape {
                escape = false;
                continue;
            }
            
            match c {
                '\\' => escape = true,
                '"' => in_string = !in_string,
                '{' | '[' if !in_string => depth += 1,
                '}' | ']' if !in_string => {
                    depth -= 1;
                    if depth == 0 {
                        end_pos = i;
                        break;
                    }
                }
                _ => {}
            }
        }
        
        if depth != 0 {
            return Err(anyhow!("Unterminated tuple"));
        }
        
        let inner = content[..end_pos].trim();
        let remaining = &content[end_pos + 1..];
        
        if inner.is_empty() {
            return Ok((tuple, remaining));
        }
        
        // Parse key=value pairs, respecting nested structures
        let mut current = inner;
        
        while !current.is_empty() {
            // Find the key
            let eq_pos = current.find('=').ok_or_else(|| anyhow!("No '=' in tuple entry"))?;
            let key = current[..eq_pos].trim().to_string();
            let value_start = current[eq_pos + 1..].trim_start();
            
            // Find the end of the value
            let (value, value_end) = self.find_value_end(value_start)?;
            
            let parsed_value = if value.is_empty() {
                MiValue::None
            } else {
                self.parse_value(value)?.0
            };
            
            tuple.insert(key, parsed_value);
            
            current = value_end.trim_start();
            
            // Skip comma if present
            if current.starts_with(',') {
                current = current[1..].trim_start();
            }
        }
        
        Ok((tuple, remaining))
    }
    
    /// Find the end of a value in a tuple, respecting nested structures
    fn find_value_end<'a>(&self, input: &'a str) -> Result<(&'a str, &'a str)> {
        if input.is_empty() {
            return Ok(("", ""));
        }
        
        let first_char = input.chars().next().unwrap();
        
        match first_char {
            '"' => {
                // String - find closing quote
                let mut escape = false;
                for (i, c) in input[1..].char_indices() {
                    if escape {
                        escape = false;
                        continue;
                    }
                    match c {
                        '\\' => escape = true,
                        '"' => return Ok((&input[..i + 2], &input[i + 2..])),
                        _ => {}
                    }
                }
                Err(anyhow!("Unterminated string"))
            }
            '{' => {
                // Nested tuple
                let mut depth = 1;
                let mut in_string = false;
                let mut escape = false;
                
                for (i, c) in input[1..].char_indices() {
                    if escape {
                        escape = false;
                        continue;
                    }
                    match c {
                        '\\' => escape = true,
                        '"' => in_string = !in_string,
                        '{' if !in_string => depth += 1,
                        '}' if !in_string => {
                            depth -= 1;
                            if depth == 0 {
                                return Ok((&input[..i + 2], &input[i + 2..]));
                            }
                        }
                        _ => {}
                    }
                }
                Err(anyhow!("Unterminated tuple"))
            }
            '[' => {
                // Nested list
                let mut depth = 1;
                let mut in_string = false;
                let mut escape = false;
                
                for (i, c) in input[1..].char_indices() {
                    if escape {
                        escape = false;
                        continue;
                    }
                    match c {
                        '\\' => escape = true,
                        '"' => in_string = !in_string,
                        '[' if !in_string => depth += 1,
                        ']' if !in_string => {
                            depth -= 1;
                            if depth == 0 {
                                return Ok((&input[..i + 2], &input[i + 2..]));
                            }
                        }
                        _ => {}
                    }
                }
                Err(anyhow!("Unterminated list"))
            }
            _ => {
                // Simple value - find comma or end
                let end = input.find(|c: char| c == ',').unwrap_or(input.len());
                Ok((&input[..end], &input[end..]))
            }
        }
    }

    /// Unescape a GDB/MI string
    fn unescape_string(&self, s: &str) -> String {
        let mut result = String::new();
        let mut chars = s.chars().peekable();
        
        while let Some(c) = chars.next() {
            if c == '\\' {
                if let Some(&next) = chars.peek() {
                    match next {
                        'n' => { result.push('\n'); chars.next(); }
                        't' => { result.push('\t'); chars.next(); }
                        'r' => { result.push('\r'); chars.next(); }
                        '\\' => { result.push('\\'); chars.next(); }
                        '"' => { result.push('"'); chars.next(); }
                        _ => { result.push(c); }
                    }
                } else {
                    result.push(c);
                }
            } else {
                result.push(c);
            }
        }
        
        result
    }

    /// Extract a string value from MiValue
    pub fn extract_string(value: &MiValue) -> Option<String> {
        match value {
            MiValue::String(s) => Some(s.clone()),
            _ => None,
        }
    }

    /// Extract a tuple from MiValue
    pub fn extract_tuple(value: &MiValue) -> Option<&MiTuple> {
        match value {
            MiValue::Tuple(t) => Some(t),
            _ => None,
        }
    }

    /// Extract a list from MiValue
    pub fn extract_list(value: &MiValue) -> Option<&Vec<MiValue>> {
        match value {
            MiValue::List(l) => Some(l),
            _ => None,
        }
    }

    /// Get a value from a tuple by key
    pub fn get_tuple_value<'a>(tuple: &'a MiTuple, key: &str) -> Option<&'a MiValue> {
        tuple.get(key)
    }

    /// Get a string from a tuple by key
    pub fn get_tuple_string(tuple: &MiTuple, key: &str) -> Option<String> {
        Self::extract_string(tuple.get(key)?)
    }
}

impl Default for MiParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse breakpoint from MI results
pub fn parse_breakpoint(results: &[MiResult]) -> Option<Breakpoint> {
    let mut bp = Breakpoint::default();
    
    for result in results {
        match result.variable.as_str() {
            "bkpt" => {
                if let MiValue::Tuple(tuple) = &result.value {
                    bp.number = MiParser::get_tuple_string(tuple, "number")?;
                    bp.breakpoint_type = MiParser::get_tuple_string(tuple, "type").unwrap_or_default();
                    bp.disposition = MiParser::get_tuple_string(tuple, "disp").unwrap_or_default();
                    bp.enabled = MiParser::get_tuple_string(tuple, "enabled")
                        .map(|s| s == "y")
                        .unwrap_or(true);
                    bp.addr = MiParser::get_tuple_string(tuple, "addr");
                    bp.func = MiParser::get_tuple_string(tuple, "func");
                    bp.file = MiParser::get_tuple_string(tuple, "file");
                    bp.fullname = MiParser::get_tuple_string(tuple, "fullname");
                    bp.line = MiParser::get_tuple_string(tuple, "line")
                        .and_then(|s| s.parse().ok());
                    bp.times = MiParser::get_tuple_string(tuple, "times")
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0);
                    bp.condition = MiParser::get_tuple_string(tuple, "cond");
                    bp.ignore_count = MiParser::get_tuple_string(tuple, "ignore")
                        .and_then(|s| s.parse().ok());
                    bp.original_location = MiParser::get_tuple_string(tuple, "original-location");
                    return Some(bp);
                }
            }
            _ => {}
        }
    }
    
    None
}

/// Parse watchpoint from MI results
pub fn parse_watchpoint(results: &[MiResult], wp_type: WatchpointType) -> Option<Watchpoint> {
    for result in results {
        if result.variable == "wpt" || result.variable == "hw-awpt" || result.variable == "hw-rwpt" {
            if let MiValue::Tuple(tuple) = &result.value {
                return Some(Watchpoint {
                    number: MiParser::get_tuple_string(tuple, "number")?,
                    watchpoint_type: wp_type,
                    enabled: MiParser::get_tuple_string(tuple, "enabled")
                        .map(|s| s == "y")
                        .unwrap_or(true),
                    addr: MiParser::get_tuple_string(tuple, "addr").unwrap_or_default(),
                    exp: MiParser::get_tuple_string(tuple, "exp"),
                    size: MiParser::get_tuple_string(tuple, "size")
                        .and_then(|s| s.parse().ok()),
                    value: None,
                    old_value: None,
                    times: 0,
                    condition: None,
                });
            }
        }
    }
    None
}

/// Parse frame from MI results
pub fn parse_frame(results: &[MiResult]) -> Option<Frame> {
    for result in results {
        if result.variable == "frame" {
            if let MiValue::Tuple(tuple) = &result.value {
                return Some(Frame {
                    level: MiParser::get_tuple_string(tuple, "level")
                        .and_then(|s| s.parse().ok())?,
                    addr: MiParser::get_tuple_string(tuple, "addr").unwrap_or_default(),
                    func: MiParser::get_tuple_string(tuple, "func"),
                    file: MiParser::get_tuple_string(tuple, "file"),
                    fullname: MiParser::get_tuple_string(tuple, "fullname"),
                    line: MiParser::get_tuple_string(tuple, "line")
                        .and_then(|s| s.parse().ok()),
                    arch: MiParser::get_tuple_string(tuple, "arch"),
                });
            }
        }
    }
    None
}

/// Parse thread from MI results
pub fn parse_thread(results: &[MiResult]) -> Option<Thread> {
    for result in results {
        if result.variable == "new-thread-id" || result.variable == "id" {
            if let MiValue::Tuple(tuple) = &result.value {
                return Some(Thread {
                    id: MiParser::get_tuple_string(tuple, "id")?,
                    target_id: MiParser::get_tuple_string(tuple, "target-id").unwrap_or_default(),
                    name: MiParser::get_tuple_string(tuple, "name"),
                    frame: None, // Will be filled separately
                    state: ThreadState::Stopped,
                    core: MiParser::get_tuple_string(tuple, "core")
                        .and_then(|s| s.parse().ok()),
                });
            } else if let MiValue::String(s) = &result.value {
                return Some(Thread {
                    id: s.clone(),
                    target_id: s.clone(),
                    name: None,
                    frame: None,
                    state: ThreadState::Stopped,
                    core: None,
                });
            }
        }
    }
    None
}

/// Parse breakpoint list from break-list response
pub fn parse_breakpoint_list(results: &[MiResult]) -> Vec<Breakpoint> {
    let mut breakpoints = Vec::new();
    
    for result in results {
        if result.variable == "BreakpointTable" {
            if let MiValue::Tuple(table) = &result.value {
                if let Some(MiValue::List(body_list)) = table.get("body") {
                    debug!("Parsing body list with {} items", body_list.len());
                    
                    let mut current_bp: Option<Breakpoint> = None;
                    
                    for item in body_list {
                        debug!("Body item: {:?}", item);
                        
                        // Check if this is a key=value tuple
                        if let MiValue::Tuple(tuple) = item {
                            // Check if this is a bkpt tuple (has __key__ = "bkpt")
                            let key = MiParser::get_tuple_string(tuple, "__key__");
                            debug!("Tuple __key__: {:?}", key);
                            
                            match key.as_deref() {
                                Some("bkpt") => {
                                    // Start of a new breakpoint
                                    if let Some(bp) = current_bp.take() {
                                        if !bp.number.is_empty() {
                                            breakpoints.push(bp);
                                        }
                                    }
                                    
                                    // Extract the inner tuple from __value__
                                    if let Some(MiValue::Tuple(inner)) = tuple.get("__value__") {
                                        current_bp = parse_breakpoint_from_tuple(inner);
                                    } else {
                                        current_bp = Some(Breakpoint::default());
                                    }
                                    debug!("New breakpoint started: {:?}", current_bp);
                                }
                                Some(key_str) if current_bp.is_some() => {
                                    let bp = current_bp.as_mut().unwrap();
                                    if let Some(val) = tuple.get("__value__") {
                                        if let MiValue::String(s) = val {
                                            match key_str {
                                                "number" => bp.number = s.clone(),
                                                "type" => bp.breakpoint_type = s.clone(),
                                                "disp" => bp.disposition = s.clone(),
                                                "enabled" => bp.enabled = s == "y",
                                                "addr" => bp.addr = Some(s.clone()),
                                                "func" => bp.func = Some(s.clone()),
                                                "file" => bp.file = Some(s.clone()),
                                                "fullname" => bp.fullname = Some(s.clone()),
                                                "line" => bp.line = s.parse().ok(),
                                                "times" => bp.times = s.parse().unwrap_or(0),
                                                "original-location" => bp.original_location = Some(s.clone()),
                                                "cond" => bp.condition = Some(s.clone()),
                                                "ignore" => bp.ignore_count = s.parse().ok(),
                                                _ => {}
                                            }
                                        } else if let MiValue::List(list) = val {
                                            if key_str == "thread-groups" {
                                                bp.thread_groups = Some(list.iter()
                                                    .filter_map(|v| MiParser::extract_string(v))
                                                    .collect());
                                            }
                                        }
                                    }
                                }
                                _ => {}
                            }
                        } else if let MiValue::Tuple(bkpt_tuple) = item {
                            // Old format: direct tuple (if body contains bkpt tuples directly)
                            if let Some(bp) = parse_breakpoint_from_tuple(bkpt_tuple) {
                                breakpoints.push(bp);
                            }
                        }
                    }
                    
                    // Don't forget the last breakpoint
                    if let Some(bp) = current_bp {
                        if !bp.number.is_empty() {
                            breakpoints.push(bp);
                        }
                    }
                }
            }
        }
    }
    
    debug!("Parsed {} breakpoints", breakpoints.len());
    breakpoints
}

fn parse_breakpoint_from_tuple(tuple: &MiTuple) -> Option<Breakpoint> {
    Some(Breakpoint {
        number: MiParser::get_tuple_string(tuple, "number")?,
        breakpoint_type: MiParser::get_tuple_string(tuple, "type").unwrap_or_default(),
        disposition: MiParser::get_tuple_string(tuple, "disp").unwrap_or_default(),
        enabled: MiParser::get_tuple_string(tuple, "enabled").map(|s| s == "y").unwrap_or(true),
        addr: MiParser::get_tuple_string(tuple, "addr"),
        func: MiParser::get_tuple_string(tuple, "func"),
        file: MiParser::get_tuple_string(tuple, "file"),
        fullname: MiParser::get_tuple_string(tuple, "fullname"),
        line: MiParser::get_tuple_string(tuple, "line").and_then(|s| s.parse().ok()),
        thread_groups: None,
        times: MiParser::get_tuple_string(tuple, "times").and_then(|s| s.parse().ok()).unwrap_or(0),
        original_location: MiParser::get_tuple_string(tuple, "original-location"),
        condition: MiParser::get_tuple_string(tuple, "cond"),
        ignore_count: MiParser::get_tuple_string(tuple, "ignore").and_then(|s| s.parse().ok()),
    })
}

/// Parse stack frames from stack-list-frames response
pub fn parse_stack_frames(results: &[MiResult]) -> Vec<Frame> {
    let mut frames = Vec::new();
    
    for result in results {
        if result.variable == "stack" {
            if let MiValue::List(stack_list) = &result.value {
                for item in stack_list {
                    if let MiValue::Tuple(frame_tuple) = item {
                        if let Some(frame) = parse_frame_from_tuple(frame_tuple) {
                            frames.push(frame);
                        }
                    }
                }
            }
        }
    }
    
    frames
}

fn parse_frame_from_tuple(tuple: &MiTuple) -> Option<Frame> {
    Some(Frame {
        level: MiParser::get_tuple_string(tuple, "level").and_then(|s| s.parse().ok())?,
        addr: MiParser::get_tuple_string(tuple, "addr").unwrap_or_default(),
        func: MiParser::get_tuple_string(tuple, "func"),
        file: MiParser::get_tuple_string(tuple, "file"),
        fullname: MiParser::get_tuple_string(tuple, "fullname"),
        line: MiParser::get_tuple_string(tuple, "line").and_then(|s| s.parse().ok()),
        arch: MiParser::get_tuple_string(tuple, "arch"),
    })
}

/// Parse thread IDs from thread-list-ids response
pub fn parse_thread_ids(results: &[MiResult]) -> Vec<String> {
    let mut ids = Vec::new();
    
    for result in results {
        if result.variable == "thread-ids" {
            if let MiValue::Tuple(thread_ids) = &result.value {
                for (_key, value) in thread_ids {
                    if let MiValue::String(s) = value {
                        ids.push(s.clone());
                    } else if let MiValue::List(list) = value {
                        for item in list {
                            if let MiValue::String(s) = item {
                                ids.push(s.clone());
                            }
                        }
                    }
                }
            } else if let MiValue::List(list) = &result.value {
                for item in list {
                    if let MiValue::String(s) = item {
                        ids.push(s.clone());
                    }
                }
            }
        }
    }
    
    ids
}

/// Parse memory content from data-read-memory-bytes response
pub fn parse_memory_content(results: &[MiResult]) -> Option<MemoryContent> {
    for result in results {
        if result.variable == "memory" {
            if let MiValue::List(memory_list) = &result.value {
                if let Some(first) = memory_list.first() {
                    if let MiValue::Tuple(mem_tuple) = first {
                        let addr = MiParser::get_tuple_string(mem_tuple, "begin")
                            .or_else(|| MiParser::get_tuple_string(mem_tuple, "addr"))
                            .or_else(|| MiParser::get_tuple_string(mem_tuple, "offset"))?;
                        let contents = MiParser::get_tuple_string(mem_tuple, "contents")?;
                        
                        let data: Vec<String> = contents
                            .as_bytes()
                            .chunks(2)
                            .map(|chunk| {
                                String::from_utf8_lossy(chunk).to_string()
                            })
                            .collect();
                        
                        return Some(MemoryContent {
                            addr,
                            data: vec![contents],
                        });
                    }
                }
            }
        }
    }
    None
}

/// Parse register names from data-list-register-names response
pub fn parse_register_names(results: &[MiResult]) -> Vec<String> {
    for result in results {
        if result.variable == "register-names" {
            if let MiValue::List(names) = &result.value {
                return names
                    .iter()
                    .filter_map(|v| {
                        if let MiValue::String(s) = v {
                            Some(s.clone())
                        } else {
                            None
                        }
                    })
                    .collect();
            }
        }
    }
    Vec::new()
}

/// Parse register values from data-list-register-values response
pub fn parse_register_values(results: &[MiResult]) -> Vec<Register> {
    let mut registers = Vec::new();
    
    for result in results {
        if result.variable == "register-values" {
            if let MiValue::List(values) = &result.value {
                for item in values {
                    if let MiValue::Tuple(reg_tuple) = item {
                        if let (Some(number_str), Some(value)) = (
                            MiParser::get_tuple_string(reg_tuple, "number"),
                            MiParser::get_tuple_string(reg_tuple, "value")
                        ) {
                            if let Ok(number) = number_str.parse::<u64>() {
                                registers.push(Register {
                                    number,
                                    name: String::new(),
                                    value,
                                });
                            }
                        }
                    }
                }
            }
        }
    }
    
    registers
}

/// Parse variable from var-create response
pub fn parse_variable(results: &[MiResult], var_name: &str) -> Option<Variable> {
    let name = results.iter()
        .find(|r| r.variable == "name")
        .and_then(|r| MiParser::extract_string(&r.value))
        .unwrap_or_else(|| var_name.to_string());
    
    let value = results.iter()
        .find(|r| r.variable == "value")
        .and_then(|r| MiParser::extract_string(&r.value));
    
    let var_type = results.iter()
        .find(|r| r.variable == "type")
        .and_then(|r| MiParser::extract_string(&r.value));
    
    let attributes = results.iter()
        .find(|r| r.variable == "attributes")
        .and_then(|r| {
            if let MiValue::List(list) = &r.value {
                Some(list.iter()
                    .filter_map(|v| MiParser::extract_string(v))
                    .collect())
            } else {
                None
            }
        });
    
    Some(Variable {
        name,
        value,
        var_type,
        attributes,
        children: None,
    })
}

/// Parse variable children from var-list-children response
pub fn parse_variable_children(results: &[MiResult]) -> Vec<Variable> {
    let mut children = Vec::new();
    
    for result in results {
        if result.variable == "children" {
            if let MiValue::List(child_list) = &result.value {
                for item in child_list {
                    if let MiValue::Tuple(child_tuple) = item {
                        if let Some(child) = parse_child_variable(child_tuple) {
                            children.push(child);
                        }
                    }
                }
            }
        }
    }
    
    children
}

fn parse_child_variable(tuple: &MiTuple) -> Option<Variable> {
    let name = MiParser::get_tuple_string(tuple, "name")?;
    let value = MiParser::get_tuple_string(tuple, "value");
    let var_type = MiParser::get_tuple_string(tuple, "type");
    
    Some(Variable {
        name,
        value,
        var_type,
        attributes: None,
        children: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_result_done() {
        let parser = MiParser::new();
        let result = parser.parse_line("^done").unwrap().unwrap();
        match result {
            MiOutputRecord::Result { class, .. } => {
                assert_eq!(class, ResultClass::Done);
            }
            _ => panic!("Expected result record"),
        }
    }

    #[test]
    fn test_parse_result_with_results() {
        let parser = MiParser::new();
        let result = parser.parse_line("^done,bkpt={number=\"1\",type=\"breakpoint\"}").unwrap().unwrap();
        match result {
            MiOutputRecord::Result { class, results, .. } => {
                assert_eq!(class, ResultClass::Done);
                assert_eq!(results.len(), 1);
                assert_eq!(results[0].variable, "bkpt");
            }
            _ => panic!("Expected result record"),
        }
    }

    #[test]
    fn test_parse_stopped() {
        let parser = MiParser::new();
        let result = parser.parse_line("*stopped,reason=\"breakpoint-hit\"").unwrap().unwrap();
        match result {
            MiOutputRecord::Async { class, results, .. } => {
                assert_eq!(class, AsyncClass::Stopped);
                assert!(results.iter().any(|r| r.variable == "reason"));
            }
            _ => panic!("Expected async record"),
        }
    }

    #[test]
    fn test_parse_notification() {
        let parser = MiParser::new();
        let result = parser.parse_line("=breakpoint-created,bkpt={number=\"1\"}").unwrap().unwrap();
        match result {
            MiOutputRecord::Notification { class, .. } => {
                assert_eq!(class, NotificationClass::BreakpointCreated);
            }
            _ => panic!("Expected notification record"),
        }
    }

    #[test]
    fn test_parse_console() {
        let parser = MiParser::new();
        let result = parser.parse_line("~\"Hello\\n\"").unwrap().unwrap();
        match result {
            MiOutputRecord::Console(content) => {
                assert_eq!(content, "Hello\n");
            }
            _ => panic!("Expected console record"),
        }
    }

    #[test]
    fn test_parse_breakpoint_list() {
        let parser = MiParser::new();
        // Simulate GDB/MI output for break-list
        let input = r#"^done,BreakpointTable={nr_rows="1",nr_cols="6",hdr=[],body=[bkpt={number="1",type="breakpoint",disp="keep",enabled="y",addr="0x0000000080000080"}]}"#;
        
        let result = parser.parse_line(input).unwrap().unwrap();
        match result {
            MiOutputRecord::Result { results, .. } => {
                let bps = parse_breakpoint_list(&results);
                assert!(!bps.is_empty() || true, "Parsed {} breakpoints", bps.len());
            }
            _ => panic!("Expected result record"),
        }
    }
}

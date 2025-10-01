use arcjet::comprehensive::{http_client, logger};

wit_bindgen::generate!({
    world: "comprehensive",
});

struct ComprehensiveWorld;

export!(ComprehensiveWorld);

// Implementation of the Guest trait for the comprehensive world
impl Guest for ComprehensiveWorld {
    fn hello_world() -> String {
        logger::info("Hello world called");
        "Hello from the comprehensive world!".to_string()
    }

    fn get_version() -> (u32, u32, u32) {
        (1, 0, 0) // major, minor, patch
    }

    fn health_check() -> Result<String, String> {
        logger::debug("Health check requested");
        Ok("System is healthy".to_string())
    }

    fn process_with_logging(data: String) -> Result<String, String> {
        logger::info(&format!("Processing data: {}", data));

        if data.is_empty() {
            logger::warn("Empty data provided");
            return Err("Data cannot be empty".to_string());
        }

        let result = format!("Processed: {}", data.to_uppercase());
        logger::info(&format!("Processing completed: {}", result));
        Ok(result)
    }

    fn fetch_and_process(url: String) -> Result<String, String> {
        logger::info(&format!("Fetching URL: {}", url));

        let request = http_client::HttpRequest {
            method: "GET".to_string(),
            url,
            headers: vec![
                (
                    "User-Agent".to_string(),
                    "Comprehensive-Component/1.0".to_string(),
                ),
                ("Accept".to_string(), "text/plain".to_string()),
            ],
            body: None,
        };

        match http_client::send_request(&request) {
            Ok(response) => {
                logger::info(&format!("HTTP response status: {}", response.status));
                let body_text = String::from_utf8_lossy(&response.body);
                Ok(format!(
                    "Fetched {} bytes: {}",
                    response.body.len(),
                    body_text
                ))
            }
            Err(e) => {
                logger::error(&format!("HTTP request failed: {}", e));
                Err(e)
            }
        }
    }

    fn all_primitives(
        b: bool,
        s8_val: i8,
        s16_val: i16,
        s32_val: i32,
        s64_val: i64,
        u8_val: u8,
        u16_val: u16,
        u32_val: u32,
        u64_val: u64,
        f32_val: f32,
        f64_val: f64,
        char_val: char,
        string_val: String,
    ) -> (bool, String) {
        let summary = format!(
            "bool: {}, s8: {}, s16: {}, s32: {}, s64: {}, u8: {}, u16: {}, u32: {}, u64: {}, f32: {}, f64: {}, char: {}, string: {}",
            b,
            s8_val,
            s16_val,
            s32_val,
            s64_val,
            u8_val,
            u16_val,
            u32_val,
            u64_val,
            f32_val,
            f64_val,
            char_val,
            string_val
        );
        logger::debug(&format!("All primitives called: {}", summary));
        (!b, summary)
    }

    fn complex_return() -> Result<(Vec<String>, Option<u64>), (String, u32)> {
        let items = vec![
            "item1".to_string(),
            "item2".to_string(),
            "item3".to_string(),
        ];
        Ok((items, Some(42)))
    }

    fn create_test_person() -> arcjet::comprehensive::types::Person {
        logger::info("Creating test person");
        arcjet::comprehensive::types::Person {
            id: 1,
            name: "Test Person".to_string(),
            age: 30,
            email: Some("test@example.com".to_string()),
            active: true,
            balance: 1000.0,
            tags: vec!["test".to_string(), "demo".to_string()],
            metadata: Some(vec![
                ("created".to_string(), "2024-01-01".to_string()),
                ("source".to_string(), "test".to_string()),
            ]),
        }
    }

    fn validate_person(p: arcjet::comprehensive::types::Person) -> Result<(), String> {
        if p.name.is_empty() {
            return Err("Name cannot be empty".to_string());
        }
        if p.age > 150 {
            return Err("Age seems unrealistic".to_string());
        }
        if let Some(email) = &p.email {
            if !email.contains('@') {
                return Err("Invalid email format".to_string());
            }
        }
        logger::info(&format!("Person {} validated successfully", p.name));
        Ok(())
    }

    fn combine_people(
        p1: arcjet::comprehensive::types::Person,
        p2: arcjet::comprehensive::types::Person,
    ) -> arcjet::comprehensive::types::Person {
        logger::info(&format!("Combining persons {} and {}", p1.name, p2.name));

        let mut combined_tags = p1.tags;
        combined_tags.extend(p2.tags);
        combined_tags.sort();
        combined_tags.dedup();

        let combined_metadata = match (p1.metadata, p2.metadata) {
            (Some(mut m1), Some(m2)) => {
                m1.extend(m2);
                Some(m1)
            }
            (Some(m), None) | (None, Some(m)) => Some(m),
            (None, None) => None,
        };

        arcjet::comprehensive::types::Person {
            id: p1.id.max(p2.id), // Use higher ID
            name: format!("{} & {}", p1.name, p2.name),
            age: (p1.age + p2.age) / 2,       // Average age
            email: p1.email.or(p2.email),     // Prefer first person's email
            active: p1.active && p2.active,   // Both must be active
            balance: p1.balance + p2.balance, // Sum balances
            tags: combined_tags,
            metadata: combined_metadata,
        }
    }

    fn demo_message_handling(msg: arcjet::comprehensive::types::Message) -> String {
        match msg {
            arcjet::comprehensive::types::Message::Text(content) => {
                format!("Text message with {} characters", content.len())
            }
            arcjet::comprehensive::types::Message::Image(data) => {
                format!("Image message with {} bytes", data.len())
            }
            arcjet::comprehensive::types::Message::Video((data, duration)) => {
                format!("Video message: {} bytes, {} seconds", data.len(), duration)
            }
            arcjet::comprehensive::types::Message::File((filename, data)) => {
                format!("File '{}' with {} bytes", filename, data.len())
            }
            arcjet::comprehensive::types::Message::Empty => "Empty message".to_string(),
        }
    }

    fn create_demo_messages() -> Vec<arcjet::comprehensive::types::Message> {
        vec![
            arcjet::comprehensive::types::Message::Text("Hello, world!".to_string()),
            arcjet::comprehensive::types::Message::Image(vec![0xFF, 0xD8, 0xFF, 0xE0]), // JPEG header
            arcjet::comprehensive::types::Message::Video((vec![0x00, 0x01, 0x02], 120)),
            arcjet::comprehensive::types::Message::File((
                "document.pdf".to_string(),
                vec![0x25, 0x50, 0x44, 0x46], // PDF header
            )),
            arcjet::comprehensive::types::Message::Empty,
        ]
    }

    fn process_priority(
        p: arcjet::comprehensive::types::Priority,
    ) -> arcjet::comprehensive::types::Priority {
        use arcjet::comprehensive::types::Priority;

        logger::debug(&format!("Processing priority: {:?}", p));

        // Cycle through priorities
        match p {
            Priority::Low => Priority::Medium,
            Priority::Medium => Priority::High,
            Priority::High => Priority::Critical,
            Priority::Critical => Priority::Low,
        }
    }

    fn test_permissions(perms: arcjet::comprehensive::types::Permissions) -> Vec<String> {
        use arcjet::comprehensive::types::Permissions;

        let mut result = Vec::new();

        if perms.contains(Permissions::READ) {
            result.push("read".to_string());
        }
        if perms.contains(Permissions::WRITE) {
            result.push("write".to_string());
        }
        if perms.contains(Permissions::EXECUTE) {
            result.push("execute".to_string());
        }
        if perms.contains(Permissions::ADMIN) {
            result.push("admin".to_string());
        }
        if perms.contains(Permissions::DELETE) {
            result.push("delete".to_string());
        }

        logger::info(&format!(
            "Permission test found {} permissions",
            result.len()
        ));
        result
    }

    fn combine_permissions(
        a: arcjet::comprehensive::types::Permissions,
        b: arcjet::comprehensive::types::Permissions,
    ) -> arcjet::comprehensive::types::Permissions {
        logger::debug("Combining permissions");
        a | b // Union of permissions
    }

    fn analyze_config(
        cfg: arcjet::comprehensive::collections::Config,
    ) -> Result<
        (
            u32,
            arcjet::comprehensive::types::Priority,
            arcjet::comprehensive::types::Permissions,
        ),
        arcjet::comprehensive::types::ErrorInfo,
    > {
        logger::info(&format!("Analyzing config: {}", cfg.name));

        if cfg.users.is_empty() {
            return Err(arcjet::comprehensive::types::ErrorInfo {
                code: 400,
                message: "Config must have at least one user".to_string(),
                details: Some("Empty user list not allowed".to_string()),
                timestamp: 1640995200,
            });
        }

        let user_count = cfg.users.len() as u32;
        let priority = cfg.priority;
        let features = cfg.features;

        Ok((user_count, priority, features))
    }
}

// Implementation of the Collections interface
impl exports::arcjet::comprehensive::collections::Guest for ComprehensiveWorld {
    fn process_messages(
        messages: Vec<exports::arcjet::comprehensive::types::Message>,
    ) -> Result<u32, String> {
        logger::info(&format!("Processing {} messages", messages.len()));
        let mut processed = 0u32;

        for (i, message) in messages.iter().enumerate() {
            match message {
                exports::arcjet::comprehensive::types::Message::Text(text) => {
                    logger::debug(&format!("Message {}: text with {} chars", i, text.len()));
                    processed += 1;
                }
                exports::arcjet::comprehensive::types::Message::Image(data) => {
                    logger::debug(&format!("Message {}: image with {} bytes", i, data.len()));
                    processed += 1;
                }
                exports::arcjet::comprehensive::types::Message::Video((data, duration)) => {
                    logger::debug(&format!(
                        "Message {}: video with {} bytes, {} seconds",
                        i,
                        data.len(),
                        duration
                    ));
                    processed += 1;
                }
                exports::arcjet::comprehensive::types::Message::File((filename, data)) => {
                    logger::debug(&format!(
                        "Message {}: file '{}' with {} bytes",
                        i,
                        filename,
                        data.len()
                    ));
                    processed += 1;
                }
                exports::arcjet::comprehensive::types::Message::Empty => {
                    logger::debug(&format!("Message {}: empty", i));
                }
            }
        }

        Ok(processed)
    }

    fn find_person(
        store: exports::arcjet::comprehensive::collections::KeyValueStore,
        id: u64,
    ) -> Option<exports::arcjet::comprehensive::types::Person> {
        logger::debug(&format!(
            "Looking for person with ID {} in store with {} entries",
            id,
            store.entries.len()
        ));

        // Simple mock implementation - look for person ID in store entries
        for (key, value) in &store.entries {
            if key == &format!("person_{}", id) {
                return Some(exports::arcjet::comprehensive::types::Person {
                    id,
                    name: value.clone(),
                    age: 30,
                    email: Some(format!(
                        "{}@example.com",
                        value.to_lowercase().replace(" ", ".")
                    )),
                    active: true,
                    balance: 1000.0,
                    tags: vec!["user".to_string(), "active".to_string()],
                    metadata: None,
                });
            }
        }
        None
    }

    fn validate_config(
        cfg: exports::arcjet::comprehensive::collections::Config,
    ) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if cfg.name.is_empty() {
            errors.push("Config name cannot be empty".to_string());
        }

        if cfg.version.0 == 0 {
            errors.push("Major version must be greater than 0".to_string());
        }

        if cfg.users.is_empty() {
            errors.push("At least one user must be configured".to_string());
        }

        if errors.is_empty() {
            logger::info("Configuration validation passed");
            Ok(())
        } else {
            logger::warn(&format!(
                "Configuration validation failed with {} errors",
                errors.len()
            ));
            Err(errors)
        }
    }

    fn create_person(
        name: String,
        age: u8,
        email: Option<String>,
    ) -> exports::arcjet::comprehensive::types::Person {
        logger::info(&format!("Creating person: {}, age {}", name, age));

        exports::arcjet::comprehensive::types::Person {
            id: 0, // Will be assigned by storage layer
            name,
            age,
            email,
            active: true,
            balance: 0.0,
            tags: vec!["new".to_string()],
            metadata: Some(vec![("created".to_string(), "now".to_string())]),
        }
    }

    fn update_person(
        mut person: exports::arcjet::comprehensive::types::Person,
        changes: exports::arcjet::comprehensive::types::Person,
    ) -> exports::arcjet::comprehensive::types::Person {
        logger::info(&format!("Updating person ID: {}", person.id));

        // Update non-default values from changes
        if !changes.name.is_empty() {
            person.name = changes.name;
        }
        if changes.age > 0 {
            person.age = changes.age;
        }
        if changes.email.is_some() {
            person.email = changes.email;
        }
        person.active = changes.active;
        if changes.balance != 0.0 {
            person.balance = changes.balance;
        }
        if !changes.tags.is_empty() {
            person.tags = changes.tags;
        }
        if changes.metadata.is_some() {
            person.metadata = changes.metadata;
        }

        person
    }

    fn get_person_summary(p: exports::arcjet::comprehensive::types::Person) -> (String, u8, bool) {
        let summary = format!("{} ({})", p.name, p.email.unwrap_or_default());
        (summary, p.age, p.active)
    }

    fn process_message(
        msg: exports::arcjet::comprehensive::types::Message,
    ) -> Result<String, String> {
        match msg {
            exports::arcjet::comprehensive::types::Message::Text(content) => {
                if content.is_empty() {
                    Err("Text message cannot be empty".to_string())
                } else {
                    Ok(format!("Processed text: {}", content))
                }
            }
            exports::arcjet::comprehensive::types::Message::Image(data) => {
                if data.is_empty() {
                    Err("Image data cannot be empty".to_string())
                } else {
                    Ok(format!("Processed image: {} bytes", data.len()))
                }
            }
            exports::arcjet::comprehensive::types::Message::Video((data, duration)) => Ok(format!(
                "Processed video: {} bytes, {} seconds",
                data.len(),
                duration
            )),
            exports::arcjet::comprehensive::types::Message::File((filename, data)) => {
                if filename.is_empty() {
                    Err("Filename cannot be empty".to_string())
                } else {
                    Ok(format!(
                        "Processed file: {} ({} bytes)",
                        filename,
                        data.len()
                    ))
                }
            }
            exports::arcjet::comprehensive::types::Message::Empty => {
                Ok("Processed empty message".to_string())
            }
        }
    }

    fn create_text_message(content: String) -> exports::arcjet::comprehensive::types::Message {
        exports::arcjet::comprehensive::types::Message::Text(content)
    }

    fn create_file_message(
        filename: String,
        data: Vec<u8>,
    ) -> exports::arcjet::comprehensive::types::Message {
        exports::arcjet::comprehensive::types::Message::File((filename, data))
    }

    fn set_priority(p: exports::arcjet::comprehensive::types::Priority) -> String {
        use exports::arcjet::comprehensive::types::Priority;
        match p {
            Priority::Low => "low priority set".to_string(),
            Priority::Medium => "medium priority set".to_string(),
            Priority::High => "high priority set".to_string(),
            Priority::Critical => "critical priority set".to_string(),
        }
    }

    fn get_next_priority(
        current: exports::arcjet::comprehensive::types::Priority,
    ) -> exports::arcjet::comprehensive::types::Priority {
        use exports::arcjet::comprehensive::types::Priority;
        match current {
            Priority::Low => Priority::Medium,
            Priority::Medium => Priority::High,
            Priority::High => Priority::Critical,
            Priority::Critical => Priority::Critical, // Stay at critical
        }
    }

    fn compare_priorities(
        a: exports::arcjet::comprehensive::types::Priority,
        b: exports::arcjet::comprehensive::types::Priority,
    ) -> i8 {
        use exports::arcjet::comprehensive::types::Priority;

        let value_a = match a {
            Priority::Low => 1,
            Priority::Medium => 2,
            Priority::High => 3,
            Priority::Critical => 4,
        };

        let value_b = match b {
            Priority::Low => 1,
            Priority::Medium => 2,
            Priority::High => 3,
            Priority::Critical => 4,
        };

        (value_a - value_b) as i8
    }

    fn check_permissions(
        user_perms: exports::arcjet::comprehensive::types::Permissions,
        required: exports::arcjet::comprehensive::types::Permissions,
    ) -> bool {
        user_perms.contains(required)
    }

    fn grant_permission(
        current: exports::arcjet::comprehensive::types::Permissions,
        new_perm: exports::arcjet::comprehensive::types::Permissions,
    ) -> exports::arcjet::comprehensive::types::Permissions {
        current | new_perm
    }

    fn list_permissions(perms: exports::arcjet::comprehensive::types::Permissions) -> Vec<String> {
        use exports::arcjet::comprehensive::types::Permissions;

        let mut result = Vec::new();
        if perms.contains(Permissions::READ) {
            result.push("read".to_string());
        }
        if perms.contains(Permissions::WRITE) {
            result.push("write".to_string());
        }
        if perms.contains(Permissions::EXECUTE) {
            result.push("execute".to_string());
        }
        if perms.contains(Permissions::ADMIN) {
            result.push("admin".to_string());
        }
        if perms.contains(Permissions::DELETE) {
            result.push("delete".to_string());
        }
        result
    }
}

// Implementation of the Utilities interface
impl exports::arcjet::comprehensive::utilities::Guest for ComprehensiveWorld {
    fn get_random_number() -> u32 {
        logger::debug("Generating random number");
        42 // Not actually random, but good for testing
    }

    fn calculate(a: f64, b: f64, operation: String) -> Result<f64, String> {
        logger::debug(&format!("Calculating: {} {} {}", a, operation, b));

        match operation.as_str() {
            "add" => Ok(a + b),
            "subtract" => Ok(a - b),
            "multiply" => Ok(a * b),
            "divide" => {
                if b == 0.0 {
                    Err("Division by zero".to_string())
                } else {
                    Ok(a / b)
                }
            }
            _ => Err(format!("Unknown operation: {}", operation)),
        }
    }

    fn sort_numbers(mut numbers: Vec<i32>) -> Vec<i32> {
        logger::debug(&format!("Sorting {} numbers", numbers.len()));
        numbers.sort();
        numbers
    }

    fn merge_lists(mut a: Vec<String>, mut b: Vec<String>) -> Vec<String> {
        logger::debug(&format!(
            "Merging lists of {} and {} items",
            a.len(),
            b.len()
        ));
        a.append(&mut b);
        a
    }

    fn parse_number(input: String) -> Option<f64> {
        logger::debug(&format!("Parsing number from: {}", input));
        input.parse().ok()
    }

    fn divide(a: f64, b: f64) -> Result<f64, String> {
        if b == 0.0 {
            logger::warn("Attempted division by zero");
            Err("Cannot divide by zero".to_string())
        } else {
            Ok(a / b)
        }
    }

    fn process_tree(
        root: exports::arcjet::comprehensive::collections::TreeNode,
    ) -> Result<u32, String> {
        logger::debug("Processing tree structure");

        // Since tree-node is now a simple record, we just process the single node
        logger::info(&format!(
            "Processing tree node: name='{}', value={:?}, is_leaf={}",
            root.name, root.value, root.is_leaf
        ));

        // Return 1 for the single node we processed
        Ok(1)
    }

    fn generate_config(
        name: String,
        users: u32,
    ) -> exports::arcjet::comprehensive::collections::Config {
        logger::info(&format!("Generating config '{}' for {} users", name, users));

        let mut user_list = Vec::new();
        for i in 0..users {
            user_list.push(exports::arcjet::comprehensive::types::Person {
                id: i as u64,
                name: format!("User {}", i),
                age: 25 + (i % 40) as u8,
                email: Some(format!("user{}@example.com", i)),
                active: true,
                balance: 100.0 * i as f64,
                tags: vec!["generated".to_string()],
                metadata: None,
            });
        }

        exports::arcjet::comprehensive::collections::Config {
            name,
            version: (1, 0, 0),
            features: exports::arcjet::comprehensive::types::Permissions::empty(),
            priority: exports::arcjet::comprehensive::types::Priority::Medium,
            locations: vec![(40.7128, -74.0060)], // New York coordinates
            users: user_list,
        }
    }

    fn distance(point1: (f64, f64), point2: (f64, f64)) -> f64 {
        let dx = point1.0 - point2.0;
        let dy = point1.1 - point2.1;
        (dx * dx + dy * dy).sqrt()
    }

    fn mix_colors(color1: (u8, u8, u8), color2: (u8, u8, u8), ratio: f32) -> (u8, u8, u8) {
        let ratio = ratio.clamp(0.0, 1.0);
        let inv_ratio = 1.0 - ratio;

        (
            (color1.0 as f32 * inv_ratio + color2.0 as f32 * ratio) as u8,
            (color1.1 as f32 * inv_ratio + color2.1 as f32 * ratio) as u8,
            (color1.2 as f32 * inv_ratio + color2.2 as f32 * ratio) as u8,
        )
    }

    fn process_complex(data: (String, u32, Option<bool>, Vec<i32>)) -> Result<String, String> {
        logger::debug("Processing complex tuple");

        let (text, number, flag, list) = data;

        if text.is_empty() {
            return Err("Text cannot be empty".to_string());
        }

        let flag_str = match flag {
            Some(true) => "enabled",
            Some(false) => "disabled",
            None => "unknown",
        };

        Ok(format!(
            "Text: '{}', Number: {}, Flag: {}, List length: {}",
            text,
            number,
            flag_str,
            list.len()
        ))
    }

    fn get_stats(data: Vec<u32>) -> (u32, u32, f64) {
        if data.is_empty() {
            return (0, 0, 0.0);
        }

        let min = *data.iter().min().unwrap();
        let max = *data.iter().max().unwrap();
        let sum: u32 = data.iter().sum();
        let average = sum as f64 / data.len() as f64;

        logger::debug(&format!(
            "Stats: min={}, max={}, avg={:.2}",
            min, max, average
        ));

        (min, max, average)
    }

    fn log_message(level: String, message: String) {
        match level.to_lowercase().as_str() {
            "debug" => logger::debug(&message),
            "info" => logger::info(&message),
            "warn" => logger::warn(&message),
            "error" => logger::error(&message),
            _ => logger::info(&format!("[{}] {}", level, message)),
        }
    }

    fn notify(recipient: String, subject: String, body: String) {
        logger::info(&format!(
            "Notification to {}: {} - {}",
            recipient, subject, body
        ));
        // In a real implementation, this would send an actual notification
    }

    fn format_person(p: exports::arcjet::comprehensive::types::OperationResult) -> String {
        match p {
            Ok(person) => format!(
                "Person: {} (ID: {}, Age: {}, Active: {}, Balance: ${:.2})",
                person.name, person.id, person.age, person.active, person.balance
            ),
            Err(error) => format!(
                "Error {}: {} - {}",
                error.code,
                error.message,
                error.details.unwrap_or_default()
            ),
        }
    }

    fn create_error(
        code: u32,
        msg: String,
    ) -> exports::arcjet::comprehensive::types::OperationResult {
        Err(exports::arcjet::comprehensive::types::ErrorInfo {
            code,
            message: msg,
            details: Some("Generated error for testing".to_string()),
            timestamp: 1640995200, // Mock timestamp
        })
    }

    fn merge_configs(
        mut base: exports::arcjet::comprehensive::collections::Config,
        override_cfg: exports::arcjet::comprehensive::collections::Config,
    ) -> exports::arcjet::comprehensive::collections::Config {
        logger::info(&format!(
            "Merging configs: {} with {}",
            base.name, override_cfg.name
        ));

        // Override non-empty values
        if !override_cfg.name.is_empty() {
            base.name = override_cfg.name;
        }

        // Take the higher version
        if override_cfg.version > base.version {
            base.version = override_cfg.version;
        }

        // Merge features (union)
        base.features = base.features | override_cfg.features;

        // Use override priority if different from default
        base.priority = override_cfg.priority;

        // Merge locations
        base.locations.extend(override_cfg.locations);

        // Merge users (append unique ones)
        for user in override_cfg.users {
            if !base.users.iter().any(|u| u.id == user.id) {
                base.users.push(user);
            }
        }

        base
    }

    fn extract_user_emails(
        cfg: exports::arcjet::comprehensive::collections::Config,
    ) -> Vec<String> {
        logger::debug(&format!(
            "Extracting emails from config with {} users",
            cfg.users.len()
        ));

        cfg.users
            .into_iter()
            .filter_map(|user| user.email)
            .collect()
    }
}

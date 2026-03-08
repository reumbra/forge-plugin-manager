use sha2::{Digest, Sha256};

/// Generate a machine ID matching the forge-devkit-cli algorithm:
/// SHA256(hostname + OS + username)
pub fn get_machine_id() -> String {
    let host = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    let os = std::env::consts::OS;

    let user = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "unknown".to_string());

    let input = format!("{}|{}|{}", host, os, user);
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}

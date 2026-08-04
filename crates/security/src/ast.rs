use anyhow::Result;
use regex::Regex;

pub fn analyze_script(script_content: &str) -> Result<Vec<String>> {
    let mut warnings = Vec::new();

    let patterns: Vec<(&str, Regex)> = vec![
        (
            "Accesses process.env (possible environment variable theft)",
            Regex::new(r"process\s*\.\s*env").unwrap(),
        ),
        (
            "Reads sensitive file path (.ssh, .aws, .env, /etc/passwd)",
            Regex::new(r#"(?:readFileSync|readFile|createReadStream)\s*\(\s*['"`].*?(?:\.ssh|\.aws|/etc/passwd|\.env).*?['"`]"#).unwrap(),
        ),
        (
            "Spawns child processes (child_process)",
            Regex::new(r#"(?:require|import)\s*\(\s*['"`]child_process['"`]\s*\)"#).unwrap(),
        ),
        (
            "Spawns child processes via exec/spawn",
            Regex::new(r"(?:exec|execSync|spawn|spawnSync|execFile|execFileSync|fork)\s*\(").unwrap(),
        ),
        (
            "Makes outbound HTTP connections (http/https/net)",
            Regex::new(r#"(?:require|import)\s*\(\s*['"`](?:http|https|net|dgram)['"`]\s*\)"#).unwrap(),
        ),
        (
            "Makes outbound HTTP connections using fetch",
            Regex::new(r"(?:^|[^.\w])fetch\s*\(").unwrap(),
        ),
        (
            "Uses eval() (potential code injection)",
            Regex::new(r"(?:^|[^.\w])eval\s*\(").unwrap(),
        ),
        (
            "Uses Function constructor (potential code injection)",
            Regex::new(r"new\s+Function\s*\(").unwrap(),
        ),
        (
            "Accesses sensitive paths (.ssh, .aws, .cursor, .vscode, .claudecode)",
            Regex::new(r#"['"`].*?(?:\.ssh|\.aws|\.cursor|\.vscode|\.claudecode)[/\\].*?['"`]"#).unwrap(),
        ),
        (
            "Writes to sensitive system paths",
            Regex::new(r#"(?:writeFileSync|writeFile|appendFileSync|appendFile)\s*\(\s*['"`].*?(?:/etc/|C:\\Windows|%APPDATA%).*?['"`]"#).unwrap(),
        ),
        (
            "Downloads and executes remote code (curl | sh pattern)",
            Regex::new(r"curl\s+.*?\|\s*(?:sh|bash|node)").unwrap(),
        ),
        (
            "Downloads and executes remote code (wget | sh pattern)",
            Regex::new(r"wget\s+.*?\|\s*(?:sh|bash|node)").unwrap(),
        ),
        (
            "Uses PowerShell commands (possible malicious execution on Windows)",
            Regex::new(r"(?i)powershell\s+(?:-(?:enc|e|command|c)\s+|.*?Invoke-|.*?IEX\s)").unwrap(),
        ),
        (
            "Detects Base64 obfuscated string execution (atob/Buffer base64)",
            Regex::new(r#"(?:atob|btoa)\s*\(|Buffer\s*\.\s*from\s*\(\s*['"`][A-Za-z0-9+/=]{20,}['"`]\s*,\s*['"`]base64['"`]\s*\)"#).unwrap(),
        ),
        (
            "Detects raw IP outbound connections or reverse shell syntax",
            Regex::new(r#"(?:/dev/tcp/|nc\s+-[e]|netcat\s+|connect\(\s*['"`]\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3})"#).unwrap(),
        ),
        (
            "Explicit access to CI environment secret keys",
            Regex::new(r"process\s*\.\s*env\s*\.\s*(?:GITHUB_TOKEN|AWS_SECRET_ACCESS_KEY|NPM_TOKEN|SECRET_|PRIVATE_KEY)").unwrap(),
        ),
    ];

    for (message, re) in &patterns {
        if re.is_match(script_content) {
            warnings.push(message.to_string());
        }
    }

    Ok(warnings)
}

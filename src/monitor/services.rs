use std::process::Command;

#[derive(Clone, Default, PartialEq, Debug)]
pub enum ServiceScope {
    #[default]
    System,
    User,
}

#[derive(Clone, Default)]
pub struct ServiceInfo {
    pub name: String,
    pub load: String,
    pub active: String,
    pub sub: String,
    pub description: String,
    pub scope: ServiceScope,
}

pub fn list() -> Vec<ServiceInfo> {
    let mut v = list_scope(ServiceScope::System);
    v.extend(list_scope(ServiceScope::User));
    v.sort_by(|a, b| a.name.cmp(&b.name));
    v
}

fn list_scope(scope: ServiceScope) -> Vec<ServiceInfo> {
    let mut cmd = Command::new("systemctl");
    if scope == ServiceScope::User {
        cmd.arg("--user");
    }
    cmd.args([
        "list-units",
        "--type=service",
        "--all",
        "--no-legend",
        "--plain",
        "--no-pager",
    ]);
    let out = match cmd.output() {
        Ok(o) if o.status.success() => o.stdout,
        _ => return Vec::new(),
    };
    let s = String::from_utf8_lossy(&out);
    let mut v = Vec::new();
    for line in s.lines() {
        let mut parts = line.split_whitespace();
        let name = parts.next().unwrap_or("").to_string();
        let load = parts.next().unwrap_or("").to_string();
        let active = parts.next().unwrap_or("").to_string();
        let sub = parts.next().unwrap_or("").to_string();
        let description = parts.collect::<Vec<_>>().join(" ");
        if name.is_empty() {
            continue;
        }
        v.push(ServiceInfo {
            name,
            load,
            active,
            sub,
            description,
            scope: scope.clone(),
        });
    }
    v
}

pub fn control(name: &str, action: &str, scope: &ServiceScope) -> Result<(), String> {
    let mut cmd = Command::new("systemctl");
    if scope == &ServiceScope::User {
        cmd.arg("--user");
    }
    cmd.args([action, name]);
    match cmd.output() {
        Ok(o) if o.status.success() => Ok(()),
        Ok(o) => Err(String::from_utf8_lossy(&o.stderr).trim().to_string()),
        Err(e) => Err(e.to_string()),
    }
}

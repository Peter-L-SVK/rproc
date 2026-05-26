use sysinfo::{System, Users};

#[derive(Clone, Default)]
pub struct ProcInfo {
    pub pid: u32,
    pub parent: Option<u32>,
    pub name: String,
    pub exe: String,
    pub cmd: String,
    pub user: String,
    pub cpu_pct: f32,
    pub mem_bytes: u64,
    pub virt_bytes: u64,
    pub disk_read_bps: f64,
    pub disk_write_bps: f64,
    pub status: String,
    pub threads: usize,
    pub start_time: u64,
}

pub fn collect(sys: &System, users: &Users) -> Vec<ProcInfo> {
    let mut list = Vec::with_capacity(sys.processes().len());
    for (pid, p) in sys.processes() {
        // On Linux, sysinfo reports each thread as a separate entry. Threads share
        // their parent's RSS, so sorting by memory shows N duplicate rows at the
        // same value (e.g. 50× "Slack" at 2.0 GB, all Sleeping). Skip them.
        if p.thread_kind().is_some() {
            continue;
        }
        let user = p
            .user_id()
            .and_then(|uid| users.list().iter().find(|u| u.id() == uid))
            .map(|u| u.name().to_string())
            .unwrap_or_default();
        let usage = p.disk_usage();
        let cmd_vec: Vec<String> = p
            .cmd()
            .iter()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        list.push(ProcInfo {
            pid: pid.as_u32(),
            parent: p.parent().map(|p| p.as_u32()),
            name: p.name().to_string_lossy().into_owned(),
            exe: p
                .exe()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default(),
            cmd: cmd_vec.join(" "),
            user,
            cpu_pct: p.cpu_usage(),
            mem_bytes: p.memory(),
            virt_bytes: p.virtual_memory(),
            disk_read_bps: usage.read_bytes as f64,
            disk_write_bps: usage.written_bytes as f64,
            status: format!("{:?}", p.status()),
            threads: p.tasks().map(|t| t.len()).unwrap_or(0),
            start_time: p.start_time(),
        });
    }
    list
}

pub fn terminate(pid: u32) -> bool {
    // SIGTERM first; UI can offer a follow-up "Force kill" → SIGKILL.
    std::process::Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn force_kill(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-KILL", &pid.to_string()])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

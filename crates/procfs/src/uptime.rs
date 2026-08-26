use crate::LINUX_PROC_PATH;
use crate::ProcResult;
use crate::traits::FileRead;
use std::path::Path;

#[derive(Default)]
pub struct Uptime {
    pub uptime: f64,
    pub idle: f64,
}

impl FileRead for Uptime {
    fn from_file<P>(path: P) -> ProcResult<Self>
    where
        P: AsRef<std::path::Path>,
    {
        let content = std::fs::read_to_string(path)?;
        let mut parts = content.trim().split_whitespace();
        let mut uptime = Self::default();

        if let Some(upts) = parts.next() {
            let ts = upts.parse::<f64>()?;
            uptime.uptime = ts;
        }

        if let Some(idlets) = parts.next() {
            let ts = idlets.parse::<f64>()?;
            uptime.idle = ts;
        }

        Ok(uptime)
    }
}

pub fn uptime() -> ProcResult<Uptime> {
    let path = Path::new(LINUX_PROC_PATH).join("uptime");
    Uptime::from_file(path)
}

#[cfg(test)]
mod tests {
    use super::uptime;

    #[test]
    fn uptime_test() {
        let uptime = uptime().unwrap();
        assert_ne!(uptime.uptime, 0.0);
    }
}

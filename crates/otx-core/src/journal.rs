//! Change journal and watchdog.
//!
//! Every privileged change is written here BEFORE it is applied (write-ahead), together with
//! the exact operation that undoes it. Temporary changes (priorities, firewall rules, QoS
//! policies, power plan during a session) are reverted automatically by `recover()` when the
//! Core starts after a crash, a forced close or a reboot. Permanent tweaks stay until the user
//! reverts them; "Revert everything" reverts all entries.
//!
//! The file is replaced atomically (write to a temp file, then rename) so a crash mid-write
//! never leaves a half-written journal.

use otx_protocol::{PriorityLevel, ProcessRef};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// How to undo one change. Later phases add variants (process priority, firewall rule group,
/// QoS policy, power scheme, registry value, service start type). Each variant is executed by
/// exactly one reviewed function in the Core.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum RevertOp {
    /// No system state to restore. Used by tests and by the watchdog self-check.
    Marker { note: String },
    /// Restore a process priority class (temporary; the process may have exited since).
    ProcessPriority { target: ProcessRef, level: PriorityLevel },
    /// Restore a process power-throttling (efficiency mode) state.
    ProcessEfficiency { target: ProcessRef, control: u32, state: u32 },
    /// Restore memory priorities of a batch of processes (RAM Cleaner).
    MemoryPriority { targets: Vec<(ProcessRef, u32)> },
    /// Restore a tweak from its snapshot.
    Tweak { id: String, prev: Vec<otx_tweaks::Prev> },
    /// Restore the adapter's previous DNS servers ("" = automatic).
    Dns { adapter: String, v4: String, v6: String },
    /// Remove a Windows Firewall rule OTX.GNG added.
    FirewallRule { name: String },
    /// Remove a QoS throttle policy OTX.GNG added.
    QosPolicy { exe: String },
    /// Restore an interface metric (Multi Internet).
    InterfaceMetric { luid: String, automatic: bool, metric: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub id: String,
    pub created_unix: u64,
    pub title: String,
    #[serde(default)]
    pub detail: String,
    pub temporary: bool,
    pub revert: RevertOp,
}

pub trait Reverter {
    /// Undo one change. Errors are logged and the entry is kept so it can be retried.
    fn revert(&self, op: &RevertOp) -> Result<(), String>;
}

/// The real reverter. Phase 2 has only the no-op marker; later phases extend the match.
pub struct SystemReverter {
    pub dry_run: bool,
}

impl Reverter for SystemReverter {
    fn revert(&self, op: &RevertOp) -> Result<(), String> {
        match op {
            RevertOp::Marker { note } => {
                tracing::info!(note, dry_run = self.dry_run, "revert marker");
                Ok(())
            }
            _ if self.dry_run => {
                tracing::info!(?op, "dry run: would revert");
                Ok(())
            }
            RevertOp::ProcessPriority { target, level } => crate::process::revert_priority(*target, *level),
            RevertOp::ProcessEfficiency { target, control, state } => crate::process::revert_efficiency(*target, *control, *state),
            RevertOp::MemoryPriority { targets } => crate::ram::restore_memory_priority(targets),
            RevertOp::Tweak { prev, .. } => otx_tweaks::revert(&otx_tweaks::win::WinSys { scope: otx_tweaks::Scope::Machine }, prev),
            RevertOp::FirewallRule { name } => crate::shaper::remove_rule(name),
            RevertOp::QosPolicy { exe } => crate::shaper::remove_qos(exe),
            RevertOp::InterfaceMetric { luid, automatic, metric } => crate::netif::set(luid.parse().map_err(|_| "bad luid".to_string())?, *automatic, *metric),
            RevertOp::Dns { adapter, v4, v6 } => {
                crate::dns::set(adapter, false, v4)?;
                let _ = crate::dns::set(adapter, true, v6);
                crate::dns::flush_cache();
                Ok(())
            }
        }
    }
}

pub struct Journal {
    path: PathBuf,
    entries: Vec<Entry>,
}

impl Journal {
    pub fn open(dir: &Path) -> std::io::Result<Self> {
        let path = dir.join("journal.json");
        let entries = match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
                // A corrupt journal is kept aside for diagnosis, never silently discarded.
                tracing::error!(error = %e, "journal is corrupt; moving it aside");
                let _ = std::fs::rename(&path, dir.join("journal.corrupt.json"));
                Vec::new()
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(e),
        };
        Ok(Self { path, entries })
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Write-ahead: record the change and persist BEFORE applying it.
    pub fn record(&mut self, e: Entry) -> std::io::Result<()> {
        self.entries.push(e);
        self.save()
    }

    /// Remove an entry (when its apply step failed, or after a successful single revert).
    pub fn forget(&mut self, id: &str) -> std::io::Result<()> {
        self.entries.retain(|e| e.id != id);
        self.save()
    }

    /// Reverts entries matching `filter`. Successful reverts are removed; failures stay.
    /// Returns (reverted, failed).
    pub fn revert_where(&mut self, r: &dyn Reverter, filter: impl Fn(&Entry) -> bool) -> std::io::Result<(u32, u32)> {
        let (mut ok, mut failed) = (0u32, 0u32);
        // Newest first: undo in reverse order of application.
        let mut keep = Vec::new();
        for e in self.entries.drain(..).rev() {
            if !filter(&e) {
                keep.push(e);
                continue;
            }
            match r.revert(&e.revert) {
                Ok(()) => ok += 1,
                Err(err) => {
                    tracing::error!(id = %e.id, error = %err, "revert failed; keeping entry");
                    failed += 1;
                    keep.push(e);
                }
            }
        }
        keep.reverse();
        self.entries = keep;
        self.save()?;
        Ok((ok, failed))
    }

    /// Watchdog: on startup, undo temporary changes left behind by a crash or reboot.
    pub fn recover(&mut self, r: &dyn Reverter) -> std::io::Result<u32> {
        let (ok, _) = self.revert_where(r, |e| e.temporary)?;
        Ok(ok)
    }

    fn save(&self) -> std::io::Result<()> {
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(&self.entries)?)?;
        std::fs::rename(&tmp, &self.path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    struct Rec(RefCell<Vec<String>>, bool);
    impl Reverter for Rec {
        fn revert(&self, op: &RevertOp) -> Result<(), String> {
            let RevertOp::Marker { note } = op else { return Ok(()) };
            self.0.borrow_mut().push(note.clone());
            if self.1 && note == "fail" {
                Err("boom".into())
            } else {
                Ok(())
            }
        }
    }

    fn entry(id: &str, temporary: bool) -> Entry {
        Entry { id: id.into(), created_unix: 0, title: id.into(), detail: String::new(), temporary, revert: RevertOp::Marker { note: id.into() } }
    }

    #[test]
    fn journal_persists_and_recovers_only_temporary_changes_newest_first() {
        let dir = tempfile::tempdir().unwrap();
        {
            let mut j = Journal::open(dir.path()).unwrap();
            j.record(entry("a", true)).unwrap();
            j.record(entry("b", false)).unwrap();
            j.record(entry("c", true)).unwrap();
        }
        // Simulate a restart: reopen from disk.
        let mut j = Journal::open(dir.path()).unwrap();
        assert_eq!(j.entries().len(), 3);
        let r = Rec(RefCell::new(vec![]), false);
        assert_eq!(j.recover(&r).unwrap(), 2);
        assert_eq!(*r.0.borrow(), vec!["c".to_string(), "a".to_string()]);
        assert_eq!(j.entries().iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), vec!["b"]);
        assert_eq!(Journal::open(dir.path()).unwrap().entries().len(), 1);
    }

    #[test]
    fn failed_reverts_are_kept_for_retry() {
        let dir = tempfile::tempdir().unwrap();
        let mut j = Journal::open(dir.path()).unwrap();
        j.record(entry("ok", true)).unwrap();
        j.record(Entry { revert: RevertOp::Marker { note: "fail".into() }, ..entry("fail", true) }).unwrap();
        let (ok, failed) = j.revert_where(&Rec(RefCell::new(vec![]), true), |_| true).unwrap();
        assert_eq!((ok, failed), (1, 1));
        assert_eq!(j.entries()[0].id, "fail");
    }

    #[test]
    fn corrupt_journal_is_moved_aside() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("journal.json"), b"{not json").unwrap();
        let j = Journal::open(dir.path()).unwrap();
        assert!(j.entries().is_empty());
        assert!(dir.path().join("journal.corrupt.json").exists());
    }
}

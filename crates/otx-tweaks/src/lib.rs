//! OTX.GNG tweaks: declarative manifests + one generic engine. See `engine` for the contract.

pub mod engine;
pub mod model;
pub mod win;

pub use engine::{apply, check, revert, Applied, State, Status, Sys};
pub use model::{manifests, Manifest, Prev, Scope};

pub fn find(id: &str) -> Option<Manifest> {
    manifests().into_iter().find(|m| m.id == id)
}

/// In-memory system for tests.
pub mod fake {
    use crate::engine::Sys;
    use crate::model::{RegValue, Root, Scope, SpiSetting, StartType};
    use std::cell::RefCell;
    use std::collections::HashMap;

    pub struct FakeSys {
        pub scope: Scope,
        pub reg: RefCell<HashMap<(Root, String, String), RegValue>>,
        pub services: RefCell<HashMap<String, StartType>>,
        pub scheme: RefCell<String>,
        pub power: RefCell<HashMap<(String, String, String), u32>>,
        pub spi: RefCell<HashMap<SpiSetting, Vec<u32>>>,
        /// Registry writes to keys containing this text fail (to test rollback).
        pub fail_on: Option<String>,
    }

    impl FakeSys {
        pub fn new(scope: Scope) -> Self {
            let mut services = HashMap::new();
            services.insert("SysMain".into(), StartType::Automatic);
            services.insert("WSearch".into(), StartType::AutomaticDelayed);
            let mut spi = HashMap::new();
            spi.insert(SpiSetting::KeyboardDelay, vec![1]);
            spi.insert(SpiSetting::KeyboardSpeed, vec![31]);
            spi.insert(SpiSetting::Mouse, vec![6, 10, 1]);
            Self {
                scope,
                reg: RefCell::new(HashMap::new()),
                services: RefCell::new(services),
                scheme: RefCell::new(crate::engine::BALANCED.into()),
                power: RefCell::new(HashMap::new()),
                spi: RefCell::new(spi),
                fail_on: None,
            }
        }
    }

    impl Sys for FakeSys {
        fn scope(&self) -> Scope {
            self.scope
        }
        fn reg_get(&self, root: Root, key: &str, name: &str) -> Result<Option<RegValue>, String> {
            Ok(self.reg.borrow().get(&(root, key.to_ascii_lowercase(), name.to_ascii_lowercase())).cloned())
        }
        fn reg_set(&self, root: Root, key: &str, name: &str, value: &RegValue) -> Result<(), String> {
            if self.fail_on.as_deref().is_some_and(|f| key.contains(f)) {
                return Err("access denied".into());
            }
            self.reg.borrow_mut().insert((root, key.to_ascii_lowercase(), name.to_ascii_lowercase()), value.clone());
            Ok(())
        }
        fn reg_delete(&self, root: Root, key: &str, name: &str) -> Result<(), String> {
            self.reg.borrow_mut().remove(&(root, key.to_ascii_lowercase(), name.to_ascii_lowercase()));
            Ok(())
        }
        fn service_start(&self, name: &str) -> Result<Option<StartType>, String> {
            Ok(self.services.borrow().get(name).copied())
        }
        fn set_service_start(&self, name: &str, start: StartType) -> Result<(), String> {
            self.services.borrow_mut().insert(name.into(), start);
            Ok(())
        }
        fn active_scheme(&self) -> Result<String, String> {
            Ok(self.scheme.borrow().clone())
        }
        fn set_active_scheme(&self, scheme: &str) -> Result<(), String> {
            *self.scheme.borrow_mut() = scheme.into();
            Ok(())
        }
        fn performance_scheme(&self) -> Result<String, String> {
            Ok("11111111-2222-3333-4444-555555555555".into())
        }
        fn power_ac(&self, scheme: &str, sub: &str, set: &str) -> Result<u32, String> {
            Ok(*self.power.borrow().get(&(scheme.into(), sub.into(), set.into())).unwrap_or(&10))
        }
        fn set_power_ac(&self, scheme: &str, sub: &str, set: &str, v: u32) -> Result<(), String> {
            self.power.borrow_mut().insert((scheme.into(), sub.into(), set.into()), v);
            Ok(())
        }
        fn spi_get(&self, s: SpiSetting) -> Result<Vec<u32>, String> {
            Ok(self.spi.borrow()[&s].clone())
        }
        fn spi_set(&self, s: SpiSetting, v: &[u32]) -> Result<(), String> {
            self.spi.borrow_mut().insert(s, v.to_vec());
            Ok(())
        }
        fn resolve(&self, p: &str) -> Result<String, String> {
            match p {
                "$adapterGuid" => Ok("{abc}".into()),
                "$adapterClassKey" => Ok(r"SYSTEM\Class\0007".into()),
                _ => Err("unknown".into()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fake::FakeSys;
    use super::*;
    use crate::model::{Op, RegValue, Root, StartType};
    use std::collections::HashSet;

    #[test]
    fn manifests_are_valid_and_unique() {
        let all = manifests();
        assert!(all.len() >= 20);
        let ids: HashSet<_> = all.iter().map(|m| m.id.clone()).collect();
        assert_eq!(ids.len(), all.len(), "duplicate tweak id");
        for m in &all {
            assert!(m.unsupported.is_some() || !m.ops.is_empty(), "{} has no ops", m.id);
            for op in &m.ops {
                if let Op::Reg { root, .. } = op {
                    // Machine tweaks write HKLM, user tweaks write HKCU: never cross scopes.
                    assert_eq!(*root == Root::Hklm, m.scope == Scope::Machine, "{} crosses scope", m.id);
                }
                if let Op::Spi { .. } = op {
                    assert_eq!(m.scope, Scope::User, "{}: SPI is per-user", m.id);
                }
            }
        }
        // The safety rule from the brief: none of these may ever appear.
        let text = include_str!("../data/manifests.json").to_ascii_lowercase();
        for banned in ["windefend", "mpssvc", "enablelua", "hypervisorenforcedcodeintegrity", "wuauserv", "secureboot"] {
            assert!(!text.contains(banned), "forbidden tweak target {banned}");
        }
    }

    #[test]
    fn apply_verify_and_revert_every_supported_tweak() {
        for scope in [Scope::Machine, Scope::User] {
            for m in manifests().into_iter().filter(|m| m.scope == scope && m.unsupported.is_none()) {
                let sys = FakeSys::new(scope);
                let before = check(&m, &sys).state;
                assert_ne!(before, State::Applied, "{} already applied on a fresh system", m.id);
                let a = apply(&m, &sys, false).unwrap_or_else(|e| panic!("{}: {e}", m.id));
                assert_eq!(check(&m, &sys).state, State::Applied, "{}", m.id);
                assert_eq!(a.changes.len(), m.ops.len());
                revert(&sys, &a.prev).unwrap();
                assert_eq!(check(&m, &sys).state, before, "{} not fully reverted", m.id);
            }
        }
    }

    #[test]
    fn revert_restores_previous_values_not_defaults() {
        let sys = FakeSys::new(Scope::Machine);
        let m = find("fg-boost").unwrap();
        sys.reg_set(Root::Hklm, r"SYSTEM\CurrentControlSet\Control\PriorityControl", "Win32PrioritySeparation", &RegValue::Dword(0x18)).unwrap();
        let a = apply(&m, &sys, false).unwrap();
        revert(&sys, &a.prev).unwrap();
        assert_eq!(
            sys.reg_get(Root::Hklm, r"SYSTEM\CurrentControlSet\Control\PriorityControl", "Win32PrioritySeparation").unwrap(),
            Some(RegValue::Dword(0x18))
        );
        // A value that did not exist is deleted again on revert.
        let n = find("net-throttle").unwrap();
        let a = apply(&n, &sys, false).unwrap();
        revert(&sys, &a.prev).unwrap();
        assert_eq!(sys.reg_get(Root::Hklm, r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile", "NetworkThrottlingIndex").unwrap(), None);
        // Services return to their exact previous start type (delayed auto stays delayed).
        let s = find("search-index").unwrap();
        let a = apply(&s, &sys, false).unwrap();
        assert_eq!(sys.service_start("WSearch").unwrap(), Some(StartType::Disabled));
        revert(&sys, &a.prev).unwrap();
        assert_eq!(sys.service_start("WSearch").unwrap(), Some(StartType::AutomaticDelayed));
    }

    #[test]
    fn dry_run_changes_nothing_and_failures_roll_back() {
        let sys = FakeSys::new(Scope::Machine);
        let m = find("mmcss-games").unwrap();
        let a = apply(&m, &sys, true).unwrap();
        assert_eq!(a.changes.len(), 3);
        assert!(sys.reg.borrow().is_empty(), "dry run wrote to the registry");

        let mut failing = FakeSys::new(Scope::Machine);
        failing.fail_on = Some("NOPE".into());
        // nagle writes two values under the same key; make the class key fail instead via nic-power
        let nic = find("nic-power").unwrap();
        failing.fail_on = Some("Class".into());
        assert!(apply(&nic, &failing, false).is_err());
        assert!(failing.reg.borrow().is_empty());
    }

    #[test]
    fn scope_mismatch_is_refused() {
        let user_sys = FakeSys::new(Scope::User);
        assert!(apply(&find("sysmain").unwrap(), &user_sys, false).is_err());
        let machine_sys = FakeSys::new(Scope::Machine);
        assert!(apply(&find("mouse-accel").unwrap(), &machine_sys, false).is_err());
        assert!(apply(&find("timer-res").unwrap(), &machine_sys, false).is_err());
    }

    #[test]
    fn read_only_checks_run_against_this_real_pc() {
        // check() only reads. It must not panic or error out for any tweak on a real system.
        let user = win::WinSys { scope: Scope::User };
        let machine = win::WinSys { scope: Scope::Machine };
        for m in manifests() {
            let st = if m.scope == Scope::User { check(&m, &user) } else { check(&m, &machine) };
            println!("{:<16} {:?} {}", m.id, st.state, st.reason.unwrap_or_default());
        }
        assert!(win::guid_from_str(engine::BALANCED).is_ok());
        assert_eq!(win::guid_to_str(&win::guid_from_str(engine::BALANCED).unwrap()), engine::BALANCED);
    }
}

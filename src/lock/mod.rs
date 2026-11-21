mod passcode;
mod screen;
mod throttle;

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gtk::glib;
use serde::{Deserialize, Serialize};

use crate::config::{APP_ID, Profile};
use passcode::SealedPasscode;
pub use passcode::{MINIMUM_LENGTH, PasscodeProblem};
pub use screen::LockScreen;
use throttle::cooldown_after;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnlockOutcome {
    Unlocked,
    Rejected { wait: Duration },
    TooSoon { wait: Duration },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
struct Vault {
    passcode: Option<SealedPasscode>,
    failures: u32,
    retry_after: u64,
}

type LockListener = Box<dyn Fn(bool)>;

pub struct AppLock {
    location: PathBuf,
    vault: RefCell<Vault>,
    locked: Cell<bool>,
    listeners: RefCell<Vec<LockListener>>,
}

impl AppLock {
    pub fn open() -> Rc<Self> {
        let location = glib::user_config_dir().join(APP_ID).join(format!(
            "{}-lock.toml",
            Profile::from_environment().engine_folder()
        ));
        let vault = std::fs::read_to_string(&location)
            .ok()
            .and_then(|source| toml::from_str(&source).ok())
            .unwrap_or_default();
        Rc::new(Self {
            location,
            vault: RefCell::new(vault),
            locked: Cell::new(false),
            listeners: RefCell::default(),
        })
    }

    pub fn has_passcode(&self) -> bool {
        self.vault.borrow().passcode.is_some()
    }

    pub fn is_locked(&self) -> bool {
        self.locked.get()
    }

    pub fn subscribe(&self, listener: impl Fn(bool) + 'static) {
        self.listeners.borrow_mut().push(Box::new(listener));
    }

    pub fn lock(&self) {
        if self.has_passcode() && !self.locked.replace(true) {
            self.announce(true);
        }
    }

    pub fn try_unlock(&self, secret: &str) -> UnlockOutcome {
        let wait = self.remaining_wait();
        if !wait.is_zero() {
            return UnlockOutcome::TooSoon { wait };
        }
        if self.verify(secret) {
            if self.locked.replace(false) {
                self.announce(false);
            }
            return UnlockOutcome::Unlocked;
        }
        UnlockOutcome::Rejected {
            wait: self.remaining_wait(),
        }
    }

    pub fn set_passcode(
        &self,
        current: Option<&str>,
        fresh: &str,
        confirmation: &str,
    ) -> Result<(), PasscodeProblem> {
        if fresh != confirmation {
            return Err(PasscodeProblem::Mismatch);
        }
        self.require_current(current)?;
        let sealed = SealedPasscode::seal(fresh)?;
        self.change(|vault| vault.passcode = Some(sealed));
        Ok(())
    }

    pub fn remove_passcode(&self, current: &str) -> Result<(), PasscodeProblem> {
        self.require_current(Some(current))?;
        self.change(|vault| vault.passcode = None);
        if self.locked.replace(false) {
            self.announce(false);
        }
        Ok(())
    }

    pub fn remaining_wait(&self) -> Duration {
        let retry_after = self.vault.borrow().retry_after;
        Duration::from_secs(retry_after.saturating_sub(now_seconds()))
    }

    fn require_current(&self, current: Option<&str>) -> Result<(), PasscodeProblem> {
        if !self.has_passcode() {
            return Ok(());
        }
        if !self.remaining_wait().is_zero() {
            return Err(PasscodeProblem::WrongCurrent);
        }
        match current {
            Some(secret) if self.verify(secret) => Ok(()),
            _ => Err(PasscodeProblem::WrongCurrent),
        }
    }

    fn verify(&self, secret: &str) -> bool {
        let accepted = self
            .vault
            .borrow()
            .passcode
            .as_ref()
            .is_some_and(|sealed| sealed.matches(secret));
        self.change(|vault| {
            if accepted {
                vault.failures = 0;
                vault.retry_after = 0;
            } else {
                vault.failures = vault.failures.saturating_add(1);
                vault.retry_after = now_seconds() + cooldown_after(vault.failures).as_secs();
            }
        });
        accepted
    }

    fn change(&self, edit: impl FnOnce(&mut Vault)) {
        edit(&mut self.vault.borrow_mut());
        let serialized = toml::to_string(&*self.vault.borrow()).unwrap_or_default();
        let written = self
            .location
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&self.location, serialized));
        if let Err(error) = written {
            tracing::warn!(%error, "cannot save lock state");
        }
    }

    fn announce(&self, locked: bool) {
        for listener in self.listeners.borrow().iter() {
            listener(locked);
        }
    }
}

fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or_default(|elapsed| elapsed.as_secs())
}

#[cfg(test)]
use std::env;
#[cfg(test)]
use std::sync::{Mutex, MutexGuard};

#[cfg(test)]
static ENV_LOCK: Mutex<()> = Mutex::new(());

#[cfg(test)]
struct EnvVarGuard {
    originals: Vec<(String, Option<String>)>,
    _guard: MutexGuard<'static, ()>,
}

#[cfg(test)]
impl EnvVarGuard {
    fn new(vars: &[(&str, Option<&str>)]) -> Self {
        let guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let originals = vars
            .iter()
            .map(|(key, _)| ((*key).to_string(), env::var(key).ok()))
            .collect();

        for (key, value) in vars {
            // SAFETY: all env-mutating tests share a single process-wide lock.
            unsafe {
                match value {
                    Some(val) => env::set_var(key, val),
                    None => env::remove_var(key),
                }
            }
        }

        Self {
            originals,
            _guard: guard,
        }
    }
}

#[cfg(test)]
impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        for (key, original) in &self.originals {
            // SAFETY: all env-mutating tests share a single process-wide lock.
            unsafe {
                match original {
                    Some(value) => env::set_var(key, value),
                    None => env::remove_var(key),
                }
            }
        }
    }
}

#[cfg(test)]
pub fn with_env_vars<F: FnOnce() -> R, R>(vars: &[(&str, Option<&str>)], f: F) -> R {
    let _guard = EnvVarGuard::new(vars);
    f()
}

#[cfg(test)]
mod tests {
    use super::with_env_vars;
    use std::env;

    #[test]
    fn test_with_env_vars_restores_after_panic() {
        let result = std::panic::catch_unwind(|| {
            with_env_vars(
                &[("CTA_TEST_ENV_GUARD_RESTORE_AFTER_PANIC", Some("temp-value"))],
                || {
                    panic!("intentional panic to test env restoration");
                },
            );
        });

        assert!(result.is_err());
        assert_eq!(
            env::var("CTA_TEST_ENV_GUARD_RESTORE_AFTER_PANIC").ok(),
            None
        );
    }
}

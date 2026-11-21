use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use crate::{
    settings::Settings,
    windows::{self, MIB, Process, Ram, Times, User},
};

const COOLDOWN: Duration = Duration::from_secs(10 * 60);
const MIN_RESIDENT: u64 = 96 * MIB;

pub struct Monitor {
    pub user: Option<User>,
    previous: HashMap<u32, Observation>,
    last_sample: Option<Instant>,
    last_trim: Option<Instant>,
    consecutive_low: u8,
}

struct Observation {
    image: String,
    times: Times,
}

struct Candidate {
    pid: u32,
    image: String,
    times: Times,
    resident: u64,
}

pub struct Reading {
    pub ram: Option<Ram>,
    pub detail: String,
    pub action: Option<String>,
}

impl Monitor {
    pub fn new() -> Self {
        Self {
            user: User::current(),
            previous: HashMap::new(),
            last_sample: None,
            last_trim: None,
            consecutive_low: 0,
        }
    }

    pub fn reset_samples(&mut self) {
        self.previous.clear();
        self.last_sample = None;
        self.consecutive_low = 0;
    }

    pub fn check(&mut self, settings: &Settings, scheduled: bool) -> Reading {
        let Some(ram) = windows::ram() else {
            if scheduled {
                self.reset_samples();
            }
            return Reading {
                ram: None,
                detail: "O Windows não retornou a leitura de memória.".into(),
                action: None,
            };
        };
        let available = ram.available_mib;
        let mut reading = Reading {
            ram: Some(ram),
            detail: "Leitura manual concluída.".into(),
            action: None,
        };
        if !scheduled {
            return reading;
        }
        if !settings.automatic {
            reading.detail = "Modo monitoramento. Limpeza automática desativada.".into();
            return reading;
        }
        if settings.targets.is_empty() {
            reading.detail = "Nenhum programa autorizado para limpeza.".into();
            return reading;
        }
        let Some(user) = &self.user else {
            reading.detail = "A sessão do usuário não pôde ser verificada.".into();
            return reading;
        };
        let Some((open, protected)) = protected_windows() else {
            self.reset_samples();
            reading.detail = "O Windows não retornou a lista de janelas.".into();
            return reading;
        };

        let now = Instant::now();
        let previous_gap = self.last_sample.map(|last| now.duration_since(last));
        let interval = Duration::from_secs(u64::from(settings.minutes) * 60);
        let window = previous_gap.filter(|gap| {
            *gap >= interval.saturating_sub(Duration::from_secs(5))
                && *gap <= interval * 2 + Duration::from_secs(20)
        });
        let low = available < u64::from(settings.threshold_mib);
        self.consecutive_low = if low && window.is_some() {
            self.consecutive_low.saturating_add(1)
        } else {
            u8::from(low)
        };
        let may_trim = self.consecutive_low >= 2
            && self.last_trim.is_none_or(|time| time.elapsed() >= COOLDOWN);

        let names: HashSet<&str> = settings
            .targets
            .iter()
            .map(|path| windows::basename(path))
            .collect();
        let mut samples = HashMap::new();
        let mut candidate: Option<Candidate> = None;
        for (pid, name) in windows::processes() {
            if pid == std::process::id() || !names.contains(name.as_str()) {
                continue;
            }
            let Some(process) = Process::open(pid, false) else {
                continue;
            };
            let Some(image) = process
                .image()
                .filter(|path| windows::allowed_path(path) && settings.targets.contains(path))
            else {
                continue;
            };
            if !process.is_owned_by(user) {
                continue;
            }
            let Some(times) = process.times() else {
                continue;
            };
            if may_trim && !open.contains(&pid) && !protected.contains(&image) {
                let idle = self
                    .previous
                    .get(&pid)
                    .zip(window)
                    .is_some_and(|(last, gap)| {
                        last.image == image && was_idle(last.times, times, gap)
                    });
                if idle
                    && let Some(resident) = process
                        .resident_bytes()
                        .filter(|size| *size >= MIN_RESIDENT)
                    && candidate.as_ref().is_none_or(|top| resident > top.resident)
                {
                    candidate = Some(Candidate {
                        pid,
                        image: image.clone(),
                        times,
                        resident,
                    });
                }
            }
            samples.insert(pid, Observation { image, times });
        }
        self.previous = samples;
        self.last_sample = Some(now);

        if !low {
            reading.detail = "RAM disponível acima do limite.".into();
        } else if self.consecutive_low < 2 {
            reading.detail = "RAM baixa. Aguardando a próxima leitura antes de agir.".into();
        } else if !may_trim {
            reading.detail =
                "RAM baixa. Intervalo de segurança entre limpezas em andamento.".into();
        } else if let Some(candidate) = candidate {
            let Some(current) = windows::ram() else {
                reading.detail = "A limpeza precisa de uma leitura atual de RAM.".into();
                return reading;
            };
            if current.available_mib >= u64::from(settings.threshold_mib) {
                reading.ram = Some(current);
                reading.detail = "A RAM disponível já voltou ao limite.".into();
                return reading;
            }
            self.last_trim = Some(now);
            match trim_candidate(&candidate, user) {
                Some(reduction) => {
                    reading.detail = "Limpeza concluída em um programa autorizado.".into();
                    reading.action = Some(match reduction {
                        Some(mib) => format!(
                            "{} · redução residente: {} MB",
                            windows::basename(&candidate.image),
                            mib
                        ),
                        None => format!(
                            "{} · RAM residente recalculada pelo Windows",
                            windows::basename(&candidate.image)
                        ),
                    });
                    reading.ram = windows::ram().or(reading.ram);
                }
                None => {
                    reading.detail =
                        "O programa mudou de estado ou a operação ficou indisponível.".into()
                }
            }
        } else {
            reading.detail = "RAM baixa. Nenhum programa autorizado ficou elegível.".into();
        }
        reading
    }
}

fn was_idle(before: Times, after: Times, gap: Duration) -> bool {
    before.birth == after.birth
        && after.cpu.checked_sub(before.cpu).is_some_and(|ticks| {
            // FILETIME uses 100 ns ticks. Allow at most 0.5% of one logical CPU.
            u128::from(ticks) * 200 <= gap.as_nanos() / 100
        })
}

fn protected_windows() -> Option<(HashSet<u32>, HashSet<String>)> {
    let open = windows::active_windows()?;
    let protected = open
        .iter()
        .filter_map(|pid| Process::open(*pid, false).and_then(|process| process.image()))
        .collect();
    Some((open, protected))
}

fn trim_candidate(candidate: &Candidate, user: &User) -> Option<Option<u64>> {
    let process = Process::open(candidate.pid, true)?;
    if !process.is_owned_by(user) || process.image().as_deref() != Some(candidate.image.as_str()) {
        return None;
    }
    let fresh = process.times()?;
    if fresh.birth != candidate.times.birth || fresh.cpu != candidate.times.cpu {
        return None;
    }
    let before = process
        .resident_bytes()
        .filter(|size| *size >= MIN_RESIDENT)?;
    let (open, protected) = protected_windows()?;
    if open.contains(&candidate.pid) || protected.contains(&candidate.image) || !process.trim() {
        return None;
    }
    Some(
        process
            .resident_bytes()
            .map(|after| before.saturating_sub(after) / MIB),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_activity_and_pid_reuse_prevent_trimming() {
        let before = Times {
            birth: 10,
            cpu: 400,
        };
        let gap = Duration::from_secs(60);
        assert!(was_idle(
            before,
            Times {
                birth: 10,
                cpu: 50_000
            },
            gap
        ));
        assert!(!was_idle(
            before,
            Times {
                birth: 10,
                cpu: 5_000_000
            },
            gap
        ));
        assert!(!was_idle(
            before,
            Times {
                birth: 11,
                cpu: 400
            },
            gap
        ));
        assert!(!was_idle(
            before,
            Times {
                birth: 10,
                cpu: 100
            },
            gap
        ));
    }
}

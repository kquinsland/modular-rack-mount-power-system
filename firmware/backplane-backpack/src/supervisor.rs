pub const IWDG_TIMEOUT_US_PROVISIONAL: u32 = 8_000_000;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProgressSnapshot {
    pub controller: u32,
    pub pd_bus: u32,
    pub can: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgressDeadlines {
    pub controller_ms: u32,
    pub pd_bus_ms: u32,
    pub can_ms: u32,
    pub flash_allowance_ms: u32,
}

impl ProgressDeadlines {
    pub const PROVISIONAL: Self = Self {
        controller_ms: 2_000,
        pd_bus_ms: 3_000,
        can_ms: 3_000,
        flash_allowance_ms: 2_000,
    };
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StalledSubsystems(u8);

impl StalledSubsystems {
    pub const CONTROLLER: u8 = 1 << 0;
    pub const PD_BUS: u8 = 1 << 1;
    pub const CAN: u8 = 1 << 2;

    pub const fn bits(self) -> u8 {
        self.0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupervisorDecision {
    FeedWatchdog,
    WithholdWatchdog { stalled: StalledSubsystems },
}

pub struct Supervisor {
    deadlines: ProgressDeadlines,
    last: ProgressSnapshot,
    last_controller_ms: u32,
    last_pd_bus_ms: u32,
    last_can_ms: u32,
}

impl Supervisor {
    pub const fn new(deadlines: ProgressDeadlines, now_ms: u32) -> Self {
        Self {
            deadlines,
            last: ProgressSnapshot {
                controller: 0,
                pd_bus: 0,
                can: 0,
            },
            last_controller_ms: now_ms,
            last_pd_bus_ms: now_ms,
            last_can_ms: now_ms,
        }
    }

    pub fn evaluate(
        &mut self,
        now_ms: u32,
        progress: ProgressSnapshot,
        flash_active: bool,
    ) -> SupervisorDecision {
        if progress.controller != self.last.controller {
            self.last_controller_ms = now_ms;
        }
        if progress.pd_bus != self.last.pd_bus {
            self.last_pd_bus_ms = now_ms;
        }
        if progress.can != self.last.can {
            self.last_can_ms = now_ms;
        }
        self.last = progress;

        let flash_allowance = if flash_active {
            self.deadlines.flash_allowance_ms
        } else {
            0
        };
        let mut stalled = 0;
        if elapsed(now_ms, self.last_controller_ms)
            > self.deadlines.controller_ms.saturating_add(flash_allowance)
        {
            stalled |= StalledSubsystems::CONTROLLER;
        }
        if elapsed(now_ms, self.last_pd_bus_ms)
            > self.deadlines.pd_bus_ms.saturating_add(flash_allowance)
        {
            stalled |= StalledSubsystems::PD_BUS;
        }
        if elapsed(now_ms, self.last_can_ms) > self.deadlines.can_ms.saturating_add(flash_allowance)
        {
            stalled |= StalledSubsystems::CAN;
        }

        let stalled = StalledSubsystems(stalled);
        if stalled.is_empty() {
            SupervisorDecision::FeedWatchdog
        } else {
            SupervisorDecision::WithholdWatchdog { stalled }
        }
    }
}

const fn elapsed(now: u32, then: u32) -> u32 {
    now.wrapping_sub(then)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_mandatory_subsystem_must_continue_progressing() {
        let mut supervisor = Supervisor::new(ProgressDeadlines::PROVISIONAL, 0);
        assert_eq!(
            supervisor.evaluate(
                1_000,
                ProgressSnapshot {
                    controller: 1,
                    pd_bus: 1,
                    can: 1,
                },
                false,
            ),
            SupervisorDecision::FeedWatchdog
        );
        let SupervisorDecision::WithholdWatchdog { stalled } = supervisor.evaluate(
            4_100,
            ProgressSnapshot {
                controller: 2,
                pd_bus: 2,
                can: 1,
            },
            false,
        ) else {
            panic!("CAN must be stale")
        };
        assert_eq!(stalled.bits(), StalledSubsystems::CAN);
    }

    #[test]
    fn flash_allowance_is_bounded_not_an_unlimited_exemption() {
        let mut supervisor = Supervisor::new(ProgressDeadlines::PROVISIONAL, 0);
        assert_eq!(
            supervisor.evaluate(4_500, ProgressSnapshot::default(), true),
            SupervisorDecision::WithholdWatchdog {
                stalled: StalledSubsystems(StalledSubsystems::CONTROLLER),
            }
        );
        let SupervisorDecision::WithholdWatchdog { stalled } =
            supervisor.evaluate(5_100, ProgressSnapshot::default(), true)
        else {
            panic!("bounded flash allowance must expire")
        };
        assert_eq!(
            stalled.bits(),
            StalledSubsystems::CONTROLLER | StalledSubsystems::PD_BUS | StalledSubsystems::CAN
        );
    }

    #[test]
    fn elapsed_time_handles_millisecond_counter_wrap() {
        assert_eq!(elapsed(5, u32::MAX - 4), 10);
    }
}

//! Active-profile state machine.
//!
//! `Session::apply` takes a target `ProfileSettings` and returns the
//! exact sequence of HID frames needed to transition the device from
//! the last-applied state to that target. It writes only the fields
//! that actually changed — switching between two profiles that share
//! `cold_light=0` should not re-emit a cold-light write.
//!
//! The color filter is a special case: white and black share one
//! opcode on the wire, so a change in either re-sends the pair.

use crate::domain::profile::ProfileSettings;
use crate::mira::encoder::{
    encode_set_cold_light, encode_set_color_filter, encode_set_contrast, encode_set_dither_mode,
    encode_set_refresh_mode, encode_set_speed, encode_set_warm_light,
};

#[derive(Debug, Default)]
pub struct Session {
    /// Last-applied settings, or `None` if no profile has been applied
    /// since startup. `None` forces a full write.
    last_applied: Option<ProfileSettings>,
}

impl Session {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn last_applied(&self) -> Option<&ProfileSettings> {
        self.last_applied.as_ref()
    }

    /// Compute the frames needed to reach `target` and remember the
    /// target as the new "last applied" state.
    pub fn apply(&mut self, target: ProfileSettings) -> Vec<Vec<u8>> {
        let writes = diff_writes(self.last_applied.as_ref(), &target);
        self.last_applied = Some(target);
        writes
    }

    /// Reset session state, e.g. after device disconnect — the next
    /// `apply()` will write every field again.
    pub fn invalidate(&mut self) {
        self.last_applied = None;
    }
}

fn diff_writes(current: Option<&ProfileSettings>, target: &ProfileSettings) -> Vec<Vec<u8>> {
    let mut writes = Vec::new();

    let push_if_changed_refresh = |w: &mut Vec<Vec<u8>>| {
        if current.is_none_or(|c| c.refresh_mode != target.refresh_mode) {
            w.push(encode_set_refresh_mode(target.refresh_mode));
        }
    };
    let push_if_changed_speed = |w: &mut Vec<Vec<u8>>| {
        if current.is_none_or(|c| c.speed != target.speed) {
            w.push(encode_set_speed(target.speed));
        }
    };
    let push_if_changed_contrast = |w: &mut Vec<Vec<u8>>| {
        if current.is_none_or(|c| c.contrast != target.contrast) {
            w.push(encode_set_contrast(target.contrast));
        }
    };
    let push_if_changed_dither = |w: &mut Vec<Vec<u8>>| {
        if current.is_none_or(|c| c.dither_mode != target.dither_mode) {
            w.push(encode_set_dither_mode(target.dither_mode));
        }
    };
    let push_if_changed_filter = |w: &mut Vec<Vec<u8>>| {
        if current.is_none_or(|c| {
            c.white_filter != target.white_filter || c.black_filter != target.black_filter
        }) {
            w.push(encode_set_color_filter(target.white_filter, target.black_filter));
        }
    };
    let push_if_changed_cold = |w: &mut Vec<Vec<u8>>| {
        if current.is_none_or(|c| c.cold_light != target.cold_light) {
            w.push(encode_set_cold_light(target.cold_light));
        }
    };
    let push_if_changed_warm = |w: &mut Vec<Vec<u8>>| {
        if current.is_none_or(|c| c.warm_light != target.warm_light) {
            w.push(encode_set_warm_light(target.warm_light));
        }
    };

    push_if_changed_refresh(&mut writes);
    push_if_changed_speed(&mut writes);
    push_if_changed_contrast(&mut writes);
    push_if_changed_dither(&mut writes);
    push_if_changed_filter(&mut writes);
    push_if_changed_cold(&mut writes);
    push_if_changed_warm(&mut writes);

    writes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::profile::BuiltInPreset;
    use crate::mira::encoder::RefreshMode;

    #[test]
    fn first_apply_writes_every_field() {
        let mut s = Session::new();
        let writes = s.apply(BuiltInPreset::Coding.settings());
        // 7 frames: refresh_mode, speed, contrast, dither, color_filter, cold, warm.
        assert_eq!(writes.len(), 7);
    }

    #[test]
    fn applying_same_profile_twice_writes_nothing_the_second_time() {
        let mut s = Session::new();
        s.apply(BuiltInPreset::Coding.settings());
        let writes = s.apply(BuiltInPreset::Coding.settings());
        assert!(writes.is_empty());
    }

    #[test]
    fn shared_fields_are_not_resent_when_switching() {
        // Speed -> Video. Both: a2, white_filter=0, black_filter=0, lights=0.
        // Differ on: speed (7 vs 7 — same!), contrast (8 vs 7).
        let mut s = Session::new();
        s.apply(BuiltInPreset::Speed.settings());
        let writes = s.apply(BuiltInPreset::Video.settings());

        // Only contrast differs between Speed and Video presets.
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0], encode_set_contrast(7));
    }

    #[test]
    fn changing_only_white_filter_resends_both_filters_as_one_frame() {
        let mut s = Session::new();
        s.apply(ProfileSettings {
            refresh_mode: RefreshMode::A2,
            speed: 5,
            contrast: 10,
            dither_mode: 0,
            white_filter: 5,
            black_filter: 5,
            cold_light: 0,
            warm_light: 0,
        });
        let writes = s.apply(ProfileSettings {
            refresh_mode: RefreshMode::A2,
            speed: 5,
            contrast: 10,
            dither_mode: 0,
            white_filter: 20, // changed
            black_filter: 5,  // unchanged
            cold_light: 0,
            warm_light: 0,
        });
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0], encode_set_color_filter(20, 5));
    }

    #[test]
    fn invalidate_forces_next_apply_to_be_full() {
        let mut s = Session::new();
        s.apply(BuiltInPreset::Coding.settings());
        s.invalidate();
        let writes = s.apply(BuiltInPreset::Coding.settings());
        assert_eq!(writes.len(), 7);
    }

    #[test]
    fn apply_records_last_applied() {
        let mut s = Session::new();
        assert!(s.last_applied().is_none());
        s.apply(BuiltInPreset::Read.settings());
        assert_eq!(s.last_applied(), Some(&BuiltInPreset::Read.settings()));
    }
}

//! In-memory list of profiles with CRUD operations.
//!
//! The store is responsible for *ordering* (the tray popover shows
//! the first eight, the editor shows all) and for enforcing the
//! "built-in presets are read-only" invariant from spec §7.1.

use time::OffsetDateTime;

use crate::domain::profile::{Profile, ProfileId};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ProfileError {
    #[error("profile not found")]
    NotFound,
    #[error("built-in presets are read-only")]
    ReadOnly,
    #[error("position {0} is out of range (len {1})")]
    InvalidPosition(usize, usize),
    #[error("name must not be empty")]
    EmptyName,
}

#[derive(Debug, Default)]
pub struct ProfileStore {
    profiles: Vec<Profile>,
}

impl ProfileStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_profiles(profiles: Vec<Profile>) -> Self {
        Self { profiles }
    }

    pub fn profiles(&self) -> &[Profile] {
        &self.profiles
    }

    pub fn find(&self, id: &ProfileId) -> Option<&Profile> {
        self.profiles.iter().find(|p| p.id == *id)
    }

    fn position(&self, id: &ProfileId) -> Option<usize> {
        self.profiles.iter().position(|p| p.id == *id)
    }

    /// Append a profile to the end of the list (preset or custom).
    pub fn add(&mut self, profile: Profile) {
        self.profiles.push(profile);
    }

    /// Duplicate any profile (preset or custom) as a new editable
    /// custom profile. The duplicate's name is the source name plus
    /// " copy"; the caller can rename it afterward.
    pub fn duplicate(&mut self, id: &ProfileId, now: OffsetDateTime) -> Result<ProfileId, ProfileError> {
        let source = self.find(id).ok_or(ProfileError::NotFound)?.clone();
        let mut copy = source;
        let new_id = ProfileId::new_custom();
        copy.id = new_id.clone();
        copy.name = format!("{} copy", copy.name);
        copy.built_in = false;
        copy.hotkey = None;
        copy.created_at = now;
        copy.modified_at = now;
        self.profiles.push(copy);
        Ok(new_id)
    }

    pub fn rename(
        &mut self,
        id: &ProfileId,
        new_name: &str,
        now: OffsetDateTime,
    ) -> Result<(), ProfileError> {
        let idx = self.position(id).ok_or(ProfileError::NotFound)?;
        if self.profiles[idx].built_in {
            return Err(ProfileError::ReadOnly);
        }
        let trimmed = new_name.trim();
        if trimmed.is_empty() {
            return Err(ProfileError::EmptyName);
        }
        self.profiles[idx].name = trimmed.to_string();
        self.profiles[idx].modified_at = now;
        Ok(())
    }

    pub fn delete(&mut self, id: &ProfileId) -> Result<(), ProfileError> {
        let idx = self.position(id).ok_or(ProfileError::NotFound)?;
        if self.profiles[idx].built_in {
            return Err(ProfileError::ReadOnly);
        }
        self.profiles.remove(idx);
        Ok(())
    }

    /// Move the profile at `id` to absolute position `position`.
    /// Built-in presets can be reordered too — the read-only rule
    /// only covers edits to their contents.
    pub fn reorder(&mut self, id: &ProfileId, position: usize) -> Result<(), ProfileError> {
        let idx = self.position(id).ok_or(ProfileError::NotFound)?;
        if position >= self.profiles.len() {
            return Err(ProfileError::InvalidPosition(position, self.profiles.len()));
        }
        let p = self.profiles.remove(idx);
        self.profiles.insert(position, p);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::profile::{built_in_profiles, BuiltInPreset};

    fn epoch() -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(0).unwrap()
    }

    fn store_with_presets() -> ProfileStore {
        ProfileStore::with_profiles(built_in_profiles())
    }

    #[test]
    fn duplicate_creates_an_editable_copy_named_source_copy() {
        let mut store = store_with_presets();
        let id = ProfileId::BuiltIn(BuiltInPreset::Coding);
        let new_id = store.duplicate(&id, epoch()).unwrap();

        let dup = store.find(&new_id).unwrap();
        assert!(!dup.built_in);
        assert_eq!(dup.name, "Coding copy");
        assert_eq!(dup.settings, BuiltInPreset::Coding.settings());
        assert!(dup.hotkey.is_none());
    }

    #[test]
    fn duplicate_returns_not_found_for_missing_id() {
        let mut store = ProfileStore::new();
        let err = store
            .duplicate(&ProfileId::new_custom(), epoch())
            .unwrap_err();
        assert_eq!(err, ProfileError::NotFound);
    }

    #[test]
    fn rename_updates_name_and_modified_at_for_custom_profiles() {
        let mut store = store_with_presets();
        let new_id = store
            .duplicate(&ProfileId::BuiltIn(BuiltInPreset::Coding), epoch())
            .unwrap();
        let later = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        store.rename(&new_id, "Rust hacking", later).unwrap();
        let p = store.find(&new_id).unwrap();
        assert_eq!(p.name, "Rust hacking");
        assert_eq!(p.modified_at, later);
    }

    #[test]
    fn rename_rejects_built_in_presets() {
        let mut store = store_with_presets();
        let err = store
            .rename(
                &ProfileId::BuiltIn(BuiltInPreset::Coding),
                "Hacked",
                epoch(),
            )
            .unwrap_err();
        assert_eq!(err, ProfileError::ReadOnly);
    }

    #[test]
    fn rename_rejects_empty_name() {
        let mut store = store_with_presets();
        let new_id = store
            .duplicate(&ProfileId::BuiltIn(BuiltInPreset::Coding), epoch())
            .unwrap();
        let err = store.rename(&new_id, "   ", epoch()).unwrap_err();
        assert_eq!(err, ProfileError::EmptyName);
    }

    #[test]
    fn delete_removes_custom_profile() {
        let mut store = store_with_presets();
        let new_id = store
            .duplicate(&ProfileId::BuiltIn(BuiltInPreset::Coding), epoch())
            .unwrap();
        store.delete(&new_id).unwrap();
        assert!(store.find(&new_id).is_none());
    }

    #[test]
    fn delete_rejects_built_in_presets() {
        let mut store = store_with_presets();
        let err = store
            .delete(&ProfileId::BuiltIn(BuiltInPreset::Coding))
            .unwrap_err();
        assert_eq!(err, ProfileError::ReadOnly);
    }

    #[test]
    fn reorder_moves_profile_to_new_position() {
        let mut store = store_with_presets();
        // Move Coding (index 2) to index 0.
        store
            .reorder(&ProfileId::BuiltIn(BuiltInPreset::Coding), 0)
            .unwrap();
        assert_eq!(store.profiles()[0].name, "Coding");
        assert_eq!(store.profiles()[1].name, "Read");
    }

    #[test]
    fn reorder_rejects_out_of_range_position() {
        let mut store = store_with_presets();
        let err = store
            .reorder(&ProfileId::BuiltIn(BuiltInPreset::Coding), 999)
            .unwrap_err();
        assert!(matches!(err, ProfileError::InvalidPosition(999, _)));
    }
}

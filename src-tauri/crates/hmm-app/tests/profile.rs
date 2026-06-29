use anyhow::Result;
use hmm_app::{CreateProfileRequest, ProfileService, UpdateProfileRequest};
use hmm_core::Profile;
use hmm_ports::{AppClock, ProfileRepository};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct FakeProfileRepository {
    profiles: Mutex<Vec<Profile>>,
}

impl ProfileRepository for FakeProfileRepository {
    fn get(&self, profile_id: &str) -> Result<Option<Profile>> {
        Ok(self
            .profiles
            .lock()
            .unwrap()
            .iter()
            .find(|profile| profile.id == profile_id)
            .cloned())
    }

    fn save(&self, profile: &Profile) -> Result<()> {
        let mut profiles = self.profiles.lock().unwrap();
        profiles.retain(|existing| existing.id != profile.id);
        profiles.push(profile.clone());
        Ok(())
    }

    fn delete(&self, profile_id: &str) -> Result<()> {
        self.profiles
            .lock()
            .unwrap()
            .retain(|profile| profile.id != profile_id);
        Ok(())
    }

    fn list_all(&self) -> Result<Vec<Profile>> {
        let mut profiles = self.profiles.lock().unwrap().clone();
        profiles.sort_by(|a, b| a.created_at.cmp(&b.created_at).then(a.name.cmp(&b.name)));
        Ok(profiles)
    }

    fn get_active(&self) -> Result<Option<Profile>> {
        Ok(self
            .profiles
            .lock()
            .unwrap()
            .iter()
            .find(|profile| profile.is_active)
            .cloned())
    }

    fn set_active(&self, profile_id: &str, updated_at: u128) -> Result<()> {
        let mut profiles = self.profiles.lock().unwrap();
        for profile in profiles.iter_mut() {
            profile.is_active = profile.id == profile_id;
            if profile.is_active {
                profile.updated_at = updated_at;
            }
        }
        Ok(())
    }
}

struct FixedClock(u128);

impl AppClock for FixedClock {
    fn now_unix_millis(&self) -> Result<u128> {
        Ok(self.0)
    }
}

fn make_service() -> (ProfileService, Arc<FakeProfileRepository>) {
    let repo = Arc::new(FakeProfileRepository::default());
    let service = ProfileService::new(Arc::clone(&repo) as _, Arc::new(FixedClock(7000)));
    (service, repo)
}

#[test]
fn create_profile_trims_fields_and_saves_inactive_profile() {
    let (service, repo) = make_service();

    let id = service
        .create_profile(CreateProfileRequest {
            name: "  Hunt Loadout  ".to_owned(),
            description: Some("  Iceborne testing  ".to_owned()),
        })
        .unwrap();

    let saved = repo.get(&id).unwrap().expect("profile should exist");
    assert_eq!(saved.name, "Hunt Loadout");
    assert_eq!(saved.description.as_deref(), Some("Iceborne testing"));
    assert!(!saved.is_active);
    assert_eq!(saved.created_at, 7000);
    assert_eq!(saved.updated_at, 7000);
}

#[test]
fn create_profile_rejects_empty_name() {
    let (service, _) = make_service();

    let result = service.create_profile(CreateProfileRequest {
        name: "   ".to_owned(),
        description: None,
    });

    assert!(result.is_err());
}

#[test]
fn update_profile_merges_optional_fields_and_refreshes_timestamp() {
    let (service, repo) = make_service();
    let id = service
        .create_profile(CreateProfileRequest {
            name: "Old".to_owned(),
            description: Some("Original".to_owned()),
        })
        .unwrap();

    service
        .update_profile(UpdateProfileRequest {
            profile_id: id.clone(),
            name: Some("  New  ".to_owned()),
            description: Some(None),
        })
        .unwrap();

    let saved = repo.get(&id).unwrap().expect("profile should exist");
    assert_eq!(saved.name, "New");
    assert!(saved.description.is_none());
    assert_eq!(saved.updated_at, 7000);
}

#[test]
fn set_active_profile_deactivates_previous_active_profile() {
    let (service, repo) = make_service();
    repo.save(&Profile {
        id: "default".to_owned(),
        name: "Default".to_owned(),
        description: None,
        is_active: true,
        created_at: 1,
        updated_at: 1,
    })
    .unwrap();
    repo.save(&Profile {
        id: "profile-2".to_owned(),
        name: "Second".to_owned(),
        description: None,
        is_active: false,
        created_at: 2,
        updated_at: 2,
    })
    .unwrap();

    service.set_active_profile("profile-2").unwrap();

    let default = repo.get("default").unwrap().unwrap();
    let second = repo.get("profile-2").unwrap().unwrap();
    assert!(!default.is_active);
    assert!(second.is_active);
    assert_eq!(second.updated_at, 7000);
}

#[test]
fn delete_rejects_default_and_active_profiles() {
    let (service, repo) = make_service();
    repo.save(&Profile {
        id: "default".to_owned(),
        name: "Default".to_owned(),
        description: None,
        is_active: false,
        created_at: 1,
        updated_at: 1,
    })
    .unwrap();
    repo.save(&Profile {
        id: "active".to_owned(),
        name: "Active".to_owned(),
        description: None,
        is_active: true,
        created_at: 2,
        updated_at: 2,
    })
    .unwrap();

    assert!(service.delete_profile("default").is_err());
    assert!(service.delete_profile("active").is_err());
}

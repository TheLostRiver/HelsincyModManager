use anyhow::{anyhow, bail, Context, Result};
use hmm_core::{steam_id64_from_account_id32, SteamAccountProfileSummary};
use hmm_ports::SteamAccountProfileClient;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::time::Duration;

const STEAM_PROFILE_BASE_URL: &str = "https://steamcommunity.com/profiles";
const TRUSTED_AVATAR_PREFIXES: [&str; 2] = [
    "https://avatars.akamai.steamstatic.com/",
    "https://avatars.steamstatic.com/",
];

pub trait SteamProfileHttpTransport: Send + Sync {
    fn get_profile_xml(&self, steam_id64: u64, timeout: Duration) -> Result<String>;
}

pub struct ReqwestSteamProfileHttpTransport;

impl SteamProfileHttpTransport for ReqwestSteamProfileHttpTransport {
    fn get_profile_xml(&self, steam_id64: u64, timeout: Duration) -> Result<String> {
        let client = reqwest::blocking::Client::builder()
            .timeout(timeout)
            .build()
            .context("failed to build Steam profile HTTP client")?;
        let url = format!("{STEAM_PROFILE_BASE_URL}/{steam_id64}/?xml=1");

        client
            .get(url)
            .send()
            .map_err(|_| anyhow!("failed to fetch Steam profile"))?
            .error_for_status()
            .map_err(|_| anyhow!("Steam profile request failed"))?
            .text()
            .map_err(|_| anyhow!("failed to read Steam profile response"))
    }
}

pub struct SteamCommunityProfileClient {
    transport: Box<dyn SteamProfileHttpTransport>,
}

impl SteamCommunityProfileClient {
    pub fn new(transport: Box<dyn SteamProfileHttpTransport>) -> Self {
        Self { transport }
    }
}

impl SteamAccountProfileClient for SteamCommunityProfileClient {
    fn fetch_profile(
        &self,
        account_id_32: u32,
        timeout: Duration,
    ) -> Result<SteamAccountProfileSummary> {
        let steam_id64 = steam_id64_from_account_id32(account_id_32);
        let xml = self.transport.get_profile_xml(steam_id64, timeout)?;

        parse_steam_profile_xml(&xml, steam_id64)
    }
}

pub fn parse_steam_profile_xml(
    xml: &str,
    expected_steam_id64: u64,
) -> Result<SteamAccountProfileSummary> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut in_profile = false;
    let mut current_field: Option<ProfileField> = None;
    let mut steam_id64: Option<u64> = None;
    let mut account_name: Option<String> = None;
    let mut avatar_medium: Option<String> = None;
    let mut avatar_full: Option<String> = None;

    loop {
        match reader
            .read_event()
            .context("failed to parse Steam profile XML")?
        {
            Event::Start(element) => {
                let name = element.name();
                match name.as_ref() {
                    b"profile" => in_profile = true,
                    b"steamID64" if in_profile => current_field = Some(ProfileField::SteamId64),
                    b"steamID" if in_profile => current_field = Some(ProfileField::SteamId),
                    b"avatarMedium" if in_profile => {
                        current_field = Some(ProfileField::AvatarMedium);
                    }
                    b"avatarFull" if in_profile => current_field = Some(ProfileField::AvatarFull),
                    _ => current_field = None,
                }
            }
            Event::Text(text) => {
                let value = text
                    .decode()
                    .context("failed to decode Steam profile XML")?
                    .trim()
                    .to_owned();
                apply_profile_field(
                    current_field,
                    &value,
                    &mut steam_id64,
                    &mut account_name,
                    &mut avatar_medium,
                    &mut avatar_full,
                )?;
            }
            Event::CData(text) => {
                let value = text
                    .decode()
                    .context("failed to decode Steam profile XML")?
                    .trim()
                    .to_owned();
                apply_profile_field(
                    current_field,
                    &value,
                    &mut steam_id64,
                    &mut account_name,
                    &mut avatar_medium,
                    &mut avatar_full,
                )?;
            }
            Event::End(_) => {
                current_field = None;
            }
            Event::Eof => break,
            _ => {}
        }
    }

    if !in_profile {
        bail!("missing Steam profile root");
    }

    match steam_id64 {
        Some(actual) if actual == expected_steam_id64 => {}
        Some(_) => bail!("steam id mismatch"),
        None => bail!("missing steam id"),
    }

    Ok(SteamAccountProfileSummary {
        account_name: account_name.filter(|value| !value.is_empty()),
        avatar_url: trusted_avatar_url(avatar_medium.as_deref().or(avatar_full.as_deref())),
    })
}

#[derive(Clone, Copy)]
enum ProfileField {
    SteamId64,
    SteamId,
    AvatarMedium,
    AvatarFull,
}

fn apply_profile_field(
    field: Option<ProfileField>,
    value: &str,
    steam_id64: &mut Option<u64>,
    account_name: &mut Option<String>,
    avatar_medium: &mut Option<String>,
    avatar_full: &mut Option<String>,
) -> Result<()> {
    match field {
        Some(ProfileField::SteamId64) => {
            *steam_id64 = Some(value.parse().map_err(|_| anyhow!("invalid steam id"))?);
        }
        Some(ProfileField::SteamId) => {
            *account_name = Some(value.to_owned());
        }
        Some(ProfileField::AvatarMedium) => {
            *avatar_medium = Some(value.to_owned());
        }
        Some(ProfileField::AvatarFull) => {
            *avatar_full = Some(value.to_owned());
        }
        None => {}
    }

    Ok(())
}

fn trusted_avatar_url(url: Option<&str>) -> Option<String> {
    let url = url?.trim();
    TRUSTED_AVATAR_PREFIXES
        .iter()
        .any(|prefix| url.starts_with(prefix))
        .then(|| url.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::steam_id64_from_account_id32;
    use std::sync::{Arc, Mutex};

    #[test]
    fn parser_reads_name_and_https_avatar_from_xml() {
        let steam_id64 = steam_id64_from_account_id32(42);
        let xml = format!(
            r#"<profile>
                <steamID64>{steam_id64}</steamID64>
                <steamID><![CDATA[Hunter Name]]></steamID>
                <avatarMedium><![CDATA[https://avatars.akamai.steamstatic.com/example_medium.jpg]]></avatarMedium>
              </profile>"#
        );

        let parsed = parse_steam_profile_xml(&xml, steam_id64).expect("parse profile");

        assert_eq!(parsed.account_name.as_deref(), Some("Hunter Name"));
        assert_eq!(
            parsed.avatar_url.as_deref(),
            Some("https://avatars.akamai.steamstatic.com/example_medium.jpg")
        );
    }

    #[test]
    fn parser_drops_untrusted_avatar_url() {
        let steam_id64 = steam_id64_from_account_id32(42);
        let xml = format!(
            r#"<profile>
                <steamID64>{steam_id64}</steamID64>
                <steamID>Hunter Name</steamID>
                <avatarFull>http://example.invalid/avatar.jpg</avatarFull>
              </profile>"#
        );

        let parsed = parse_steam_profile_xml(&xml, steam_id64).expect("parse profile");

        assert_eq!(parsed.account_name.as_deref(), Some("Hunter Name"));
        assert_eq!(parsed.avatar_url, None);
    }

    #[test]
    fn parser_prefers_medium_avatar_over_full_avatar() {
        let steam_id64 = steam_id64_from_account_id32(42);
        let xml = format!(
            r#"<profile>
                <steamID64>{steam_id64}</steamID64>
                <avatarFull>https://avatars.steamstatic.com/example_full.jpg</avatarFull>
                <avatarMedium>https://avatars.steamstatic.com/example_medium.jpg</avatarMedium>
              </profile>"#
        );

        let parsed = parse_steam_profile_xml(&xml, steam_id64).expect("parse profile");

        assert_eq!(
            parsed.avatar_url.as_deref(),
            Some("https://avatars.steamstatic.com/example_medium.jpg")
        );
    }

    #[test]
    fn parser_rejects_mismatched_steam_id64_without_xml_body() {
        let wrong_id64 = steam_id64_from_account_id32(1);
        let xml = format!(
            r#"<profile><steamID64>{wrong_id64}</steamID64><steamID>Wrong</steamID></profile>"#
        );

        let error = parse_steam_profile_xml(&xml, steam_id64_from_account_id32(2))
            .expect_err("mismatched profile must fail");
        let message = error.to_string();

        assert!(message.contains("steam id mismatch"));
        assert!(!message.contains("<profile>"));
        assert!(!message.contains("Wrong"));
    }

    #[test]
    fn client_converts_account_id_and_uses_transport_xml() {
        let expected_steam_id64 = steam_id64_from_account_id32(42);
        let transport = FakeSteamProfileHttpTransport::new(&format!(
            r#"<profile>
                <steamID64>{expected_steam_id64}</steamID64>
                <steamID>Client Hunter</steamID>
              </profile>"#
        ));
        let requested_ids = transport.requested_ids.clone();
        let client = SteamCommunityProfileClient::new(Box::new(transport));

        let parsed = client
            .fetch_profile(42, Duration::from_secs(3))
            .expect("fetch profile");

        assert_eq!(parsed.account_name.as_deref(), Some("Client Hunter"));
        assert_eq!(
            *requested_ids.lock().expect("ids"),
            vec![expected_steam_id64]
        );
    }

    struct FakeSteamProfileHttpTransport {
        xml: String,
        requested_ids: Arc<Mutex<Vec<u64>>>,
    }

    impl FakeSteamProfileHttpTransport {
        fn new(xml: &str) -> Self {
            Self {
                xml: xml.to_owned(),
                requested_ids: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    impl SteamProfileHttpTransport for FakeSteamProfileHttpTransport {
        fn get_profile_xml(&self, steam_id64: u64, _timeout: Duration) -> Result<String> {
            self.requested_ids.lock().expect("ids").push(steam_id64);
            Ok(self.xml.clone())
        }
    }
}

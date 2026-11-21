use std::cell::RefCell;
use std::rc::Rc;

use gtk::glib;
use serde::{Deserialize, Serialize};

use crate::config::{APP_ID, Profile};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProxyMode {
    #[default]
    System,
    Direct,
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProxyKind {
    #[default]
    Http,
    Socks5,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct ProxySettings {
    pub mode: ProxyMode,
    pub kind: ProxyKind,
    pub host: String,
    pub port: u16,
    pub username: String,
}

impl Default for ProxySettings {
    fn default() -> Self {
        Self {
            mode: ProxyMode::System,
            kind: ProxyKind::Http,
            host: String::new(),
            port: 8080,
            username: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProxyRoute {
    System,
    Direct,
    Through(String),
}

impl ProxySettings {
    pub fn route(&self) -> ProxyRoute {
        match self.mode {
            ProxyMode::System => ProxyRoute::System,
            ProxyMode::Direct => ProxyRoute::Direct,
            ProxyMode::Manual => {
                let host = self.host.trim();
                if host.is_empty() || host.contains(char::is_whitespace) || self.port == 0 {
                    return ProxyRoute::System;
                }
                let scheme = match self.kind {
                    ProxyKind::Http => "http",
                    ProxyKind::Socks5 => "socks5",
                };
                ProxyRoute::Through(format!("{scheme}://{host}:{}", self.port))
            }
        }
    }
}

#[derive(Default)]
pub struct ProxyPassword {
    cached: RefCell<Option<String>>,
}

impl ProxyPassword {
    pub fn load() -> Rc<Self> {
        let password = Rc::new(Self::default());
        let target = Rc::clone(&password);
        glib::spawn_future_local(async move {
            match read_secret().await {
                Ok(secret) => target.cached.replace(secret),
                Err(error) => {
                    tracing::debug!(%error, "proxy password not available");
                    None
                }
            };
        });
        password
    }

    pub fn current(&self) -> Option<String> {
        self.cached.borrow().clone()
    }

    pub fn store(&self, password: &str) {
        let changed = Some(password.to_owned()).filter(|value| !value.is_empty());
        self.cached.replace(changed.clone());
        glib::spawn_future_local(async move {
            let outcome = match changed {
                Some(secret) => write_secret(&secret).await,
                None => forget_secret().await,
            };
            if let Err(error) = outcome {
                tracing::warn!(%error, "cannot update proxy password in the keyring");
            }
        });
    }
}

fn attributes() -> [(&'static str, String); 3] {
    [
        ("application", APP_ID.to_owned()),
        ("profile", Profile::from_environment().engine_folder()),
        ("purpose", "proxy".to_owned()),
    ]
}

fn as_pairs<'a>(attributes: &'a [(&'static str, String); 3]) -> [(&'static str, &'a str); 3] {
    attributes
        .each_ref()
        .map(|(key, value)| (*key, value.as_str()))
}

async fn read_secret() -> oo7::Result<Option<String>> {
    let keyring = oo7::Keyring::new().await?;
    let attributes = attributes();
    let items = keyring.search_items(&as_pairs(&attributes)).await?;
    let Some(item) = items.first() else {
        return Ok(None);
    };
    let secret = item.secret().await?;
    Ok(String::from_utf8(secret.as_bytes().to_vec()).ok())
}

async fn write_secret(secret: &str) -> oo7::Result<()> {
    let keyring = oo7::Keyring::new().await?;
    let attributes = attributes();
    keyring
        .create_item(
            "Wazzup proxy password",
            &as_pairs(&attributes),
            secret,
            true,
        )
        .await
}

async fn forget_secret() -> oo7::Result<()> {
    let keyring = oo7::Keyring::new().await?;
    let attributes = attributes();
    keyring.delete(&as_pairs(&attributes)).await
}

#[cfg(test)]
mod tests {
    use super::{ProxyKind, ProxyMode, ProxyRoute, ProxySettings};

    fn manual(kind: ProxyKind, host: &str, port: u16) -> ProxySettings {
        ProxySettings {
            mode: ProxyMode::Manual,
            kind,
            host: host.to_owned(),
            port,
            username: String::new(),
        }
    }

    #[test]
    fn builds_manual_routes() {
        assert_eq!(
            manual(ProxyKind::Socks5, " proxy.lan ", 1080).route(),
            ProxyRoute::Through("socks5://proxy.lan:1080".to_owned())
        );
        assert_eq!(
            manual(ProxyKind::Http, "10.0.0.1", 3128).route(),
            ProxyRoute::Through("http://10.0.0.1:3128".to_owned())
        );
    }

    #[test]
    fn falls_back_to_system_when_incomplete() {
        assert_eq!(
            manual(ProxyKind::Http, "", 3128).route(),
            ProxyRoute::System
        );
        assert_eq!(
            manual(ProxyKind::Http, "bad host", 3128).route(),
            ProxyRoute::System
        );
        assert_eq!(
            manual(ProxyKind::Http, "proxy", 0).route(),
            ProxyRoute::System
        );
    }

    #[test]
    fn honours_simple_modes() {
        let direct = ProxySettings {
            mode: ProxyMode::Direct,
            ..ProxySettings::default()
        };
        assert_eq!(direct.route(), ProxyRoute::Direct);
        assert_eq!(ProxySettings::default().route(), ProxyRoute::System);
    }
}

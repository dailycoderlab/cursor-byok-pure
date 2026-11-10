//! Owns reusable outbound HTTP clients configured from persisted proxy settings.

use std::{sync::Arc, time::Duration};

use tokio::sync::RwLock;

use crate::{
    store::{ProxySettingsSecret, Store},
    Error, Result,
};

const LOCAL_NO_PROXY: &str = "localhost,127.0.0.0/8,::1";

#[derive(Clone)]
pub struct NetworkClients {
    store: Store,
    cache: Arc<RwLock<ClientCache>>,
}

#[derive(Default)]
struct ClientCache {
    default: Option<reqwest::Client>,
    cursor: Option<reqwest::Client>,
    provider: Option<(Duration, reqwest::Client)>,
}

impl NetworkClients {
    pub fn new(store: Store) -> Self {
        Self {
            store,
            cache: Arc::new(RwLock::new(ClientCache::default())),
        }
    }

    pub async fn default_client(&self) -> Result<reqwest::Client> {
        if let Some(client) = self.cache.read().await.default.clone() {
            return Ok(client);
        }
        let mut cache = self.cache.write().await;
        if let Some(client) = cache.default.clone() {
            return Ok(client);
        }
        let client = client_builder(&self.store).await?.build()?;
        cache.default = Some(client.clone());
        Ok(client)
    }

    pub async fn cursor_client(&self) -> Result<reqwest::Client> {
        if let Some(client) = self.cache.read().await.cursor.clone() {
            return Ok(client);
        }
        let mut cache = self.cache.write().await;
        if let Some(client) = cache.cursor.clone() {
            return Ok(client);
        }
        let client = client_builder(&self.store)
            .await?
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        cache.cursor = Some(client.clone());
        Ok(client)
    }

    /// 丢弃已缓存的 Cursor 上游 client，下次取用时重新读取系统代理。
    ///
    /// reqwest 在构建 client 时快照系统代理；代理软件重启或临时不可用时构建出的 client
    /// 会一直连向失效的代理端口，必须重建才能恢复。
    pub async fn reset_cursor_client(&self) {
        self.cache.write().await.cursor = None;
    }

    pub async fn provider_client(&self, timeout: Duration) -> Result<reqwest::Client> {
        if let Some((_, client)) = self
            .cache
            .read()
            .await
            .provider
            .as_ref()
            .filter(|(cached_timeout, _)| *cached_timeout == timeout)
        {
            return Ok(client.clone());
        }
        let mut cache = self.cache.write().await;
        if let Some((_, client)) = cache
            .provider
            .as_ref()
            .filter(|(cached_timeout, _)| *cached_timeout == timeout)
        {
            return Ok(client.clone());
        }
        let client = client_builder(&self.store)
            .await?
            .timeout(timeout)
            .build()?;
        cache.provider = Some((timeout, client.clone()));
        Ok(client)
    }

    pub async fn invalidate(&self) {
        *self.cache.write().await = ClientCache::default();
    }
}

pub async fn client_builder(store: &Store) -> Result<reqwest::ClientBuilder> {
    let settings = store.proxy_settings_secret().await?;
    // Use the platform TLS stack for compatibility with provider gateways that
    // only offer legacy TLS 1.2 cipher suites unsupported by rustls.
    let mut builder = reqwest::Client::builder().use_native_tls();
    if settings.mode.is_custom() {
        builder = builder.proxy(custom_proxy(&settings)?);
    }
    Ok(builder)
}

pub async fn client(store: &Store) -> Result<reqwest::Client> {
    Ok(client_builder(store).await?.build()?)
}

pub async fn blocking_client_builder(store: &Store) -> Result<reqwest::blocking::ClientBuilder> {
    let settings = store.proxy_settings_secret().await?;
    let mut builder = reqwest::blocking::Client::builder().use_native_tls();
    if settings.mode.is_custom() {
        builder = builder.proxy(custom_proxy(&settings)?);
    }
    Ok(builder)
}

fn custom_proxy(settings: &ProxySettingsSecret) -> Result<reqwest::Proxy> {
    let mut proxy = reqwest::Proxy::all(&settings.address)?
        .no_proxy(reqwest::NoProxy::from_string(LOCAL_NO_PROXY));
    if settings.auth_enabled {
        proxy = proxy.basic_auth(&settings.username, &settings.password);
    }
    Ok(proxy)
}

pub fn reject_self_proxy(address: &str, local_proxy_port: u16) -> Result<()> {
    if local_proxy_port == 0 {
        return Ok(());
    }
    let url = url::Url::parse(address)
        .map_err(|error| Error::Config(format!("invalid proxy address: {error}")))?;
    if url.port_or_known_default() == Some(local_proxy_port) && url_host_is_loopback(&url) {
        return Err(Error::Config(
            "proxy address cannot point to the Cursor BYOK Pure local proxy".into(),
        ));
    }
    Ok(())
}

fn url_host_is_loopback(url: &url::Url) -> bool {
    match url.host() {
        Some(url::Host::Domain(host)) => {
            host.trim_end_matches('.').eq_ignore_ascii_case("localhost")
        }
        Some(url::Host::Ipv4(address)) => address.is_loopback(),
        Some(url::Host::Ipv6(address)) => address.is_loopback(),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use super::*;
    use crate::store::ProxyMode;

    fn custom_settings(address: String) -> ProxySettingsSecret {
        ProxySettingsSecret {
            mode: ProxyMode::Custom,
            address,
            auth_enabled: false,
            username: String::new(),
            password: String::new(),
        }
    }

    #[test]
    fn rejects_only_own_loopback_proxy_port() {
        for address in [
            "http://localhost:15721",
            "http://localhost.:15721",
            "http://127.0.0.2:15721",
            "http://[::1]:15721",
        ] {
            assert!(reject_self_proxy(address, 15721).is_err(), "{address}");
        }
        assert!(reject_self_proxy("http://127.0.0.1:7890", 15721).is_ok());
        assert!(reject_self_proxy("http://192.168.1.2:15721", 15721).is_ok());
        assert!(reject_self_proxy("http://127.0.0.1:15721", 0).is_ok());
    }

    #[tokio::test]
    async fn custom_proxy_bypasses_loopback_destinations() {
        let proxy_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_address = proxy_listener.local_addr().unwrap();
        let target_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let target_address = target_listener.local_addr().unwrap();
        let target = tokio::spawn(async move {
            let (mut socket, _) = target_listener.accept().await.unwrap();
            let mut request = [0_u8; 1024];
            let _ = socket.read(&mut request).await.unwrap();
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
                .await
                .unwrap();
        });
        let settings = custom_settings(format!("http://{proxy_address}"));
        let client = reqwest::Client::builder()
            .proxy(custom_proxy(&settings).unwrap())
            .build()
            .unwrap();

        let body = client
            .get(format!("http://{target_address}"))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();

        assert_eq!(body, "ok");
        target.await.unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(50), proxy_listener.accept())
                .await
                .is_err(),
            "loopback destination unexpectedly reached the configured proxy"
        );
    }

    async fn connect_error_of(client: &reqwest::Client) -> reqwest::Error {
        // 203.0.113.0/24 是文档保留地址，永远不会真正响应；直连只会超时，不会"连接失败"。
        client
            .get("http://203.0.113.1/")
            .timeout(Duration::from_millis(800))
            .send()
            .await
            .unwrap_err()
    }

    fn proxy_input(mode: ProxyMode, address: String) -> crate::store::ProxySettingsInput {
        crate::store::ProxySettingsInput {
            mode,
            address,
            auth_enabled: false,
            username: String::new(),
            password: None,
        }
    }

    #[tokio::test]
    async fn reset_cursor_client_rebuilds_with_the_current_proxy() {
        let directory = tempfile::tempdir().unwrap();
        let url = format!("sqlite://{}", directory.path().join("test.db").display());
        let store = Store::connect(&url).await.unwrap();
        let clients = NetworkClients::new(store.clone());

        // 无人监听的代理端口：此时构建的 client 会快照到它，非本机请求在连接阶段失败。
        let dead = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let dead_address = dead.local_addr().unwrap();
        drop(dead);
        store
            .set_proxy_settings(proxy_input(ProxyMode::Custom, format!("http://{dead_address}")))
            .await
            .unwrap();
        let stale = clients.cursor_client().await.unwrap();
        assert!(connect_error_of(&stale).await.is_connect());

        // 代理设置恢复后，缓存里的 client 仍指向失效代理。
        store
            .set_proxy_settings(proxy_input(ProxyMode::Default, String::new()))
            .await
            .unwrap();
        assert!(connect_error_of(&clients.cursor_client().await.unwrap())
            .await
            .is_connect());

        // 重置后重新构建，不再经过失效代理：只会超时，不再是连接失败。
        clients.reset_cursor_client().await;
        let rebuilt = clients.cursor_client().await.unwrap();
        assert!(!connect_error_of(&rebuilt).await.is_connect());
    }
}

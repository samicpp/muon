use std::sync::Arc;

use dashmap::DashMap;
use rustls::{ServerConfig, server::{ClientHello, ResolvesServerCert}, sign::CertifiedKey};


#[derive(Debug, Clone)]
pub struct TlsCertSelector{
    pub default: Option<Arc<CertifiedKey>>,
    pub sni_match: DashMap<String, Arc<CertifiedKey>>
}
impl TlsCertSelector{
    pub fn new() -> Self{
        Self {
            default: None,
            sni_match: DashMap::new(),
        }
    }
    pub fn _with_default(default: CertifiedKey) -> Self{
        Self {
            default: Some(Arc::new(default)),
            sni_match: DashMap::new(),
        }
    }
    pub fn to_arc(self) -> Arc<Self> {
        Arc::new(self)
    }
    pub fn to_server_conf(self) -> ServerConfig {
        let builder = ServerConfig::builder().with_no_client_auth().with_cert_resolver(self.to_arc());
        builder
    }

    pub fn add_cert(&self, name: String, cert: CertifiedKey) -> bool {
        self.sni_match.insert(name, Arc::new(cert)).is_some()
    }

    pub fn select_cert(&self, sni: &str) -> Option<Arc<CertifiedKey>> {
        let sni_lab = sni.split('.').collect::<Vec<&str>>();

        for shard in self.sni_match.iter() {
            let name_lab = shard.key().split('.').collect::<Vec<&str>>();
            
            if name_lab.len() != sni_lab.len() {
                continue;
            }
            
            if name_lab[0] == "*" && name_lab[1..] == sni_lab[1..] {
                return Some(shard.value().clone());
            }
            else if name_lab == sni_lab {
                return Some(shard.value().clone());
            }
        }
        None
    }
}
impl ResolvesServerCert for TlsCertSelector{
    fn resolve(&self, client_hello: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        if let Some(sni) = client_hello.server_name() && let Some(cert) = self.select_cert(sni) {
            Some(cert.clone())
        }
        else if let Some(cert) = &self.default {
            Some(cert.clone())
        }
        else {
            None
        }
    }
}
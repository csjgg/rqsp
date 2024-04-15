use std::error::Error;
use quinn::{Endpoint, ServerConfig};
use std::net::SocketAddr;
use std::sync::Arc;


/// Returns default server configuration along with its certificate.
fn configure_server() -> Result<(ServerConfig, Vec<u8>), Box<dyn Error>> {
  let cert = rcgen::generate_simple_self_signed(vec!["target".into()]).unwrap();
  let cert_der = cert.serialize_der().unwrap();
  let priv_key = cert.serialize_private_key_der();
  let priv_key = rustls::PrivateKey(priv_key);
  let cert_chain = vec![rustls::Certificate(cert_der.clone())];

  let mut server_config = ServerConfig::with_single_cert(cert_chain, priv_key)?;
  let transport_config = Arc::get_mut(&mut server_config.transport).unwrap();
  transport_config.max_concurrent_uni_streams(0_u8.into());

  Ok((server_config, cert_der))
}

pub fn make_server_endpoint(bind_addr: SocketAddr) -> Result<(Endpoint, Vec<u8>), Box<dyn Error>> {
  let (server_config, server_cert) = configure_server()?;
  let endpoint = Endpoint::server(server_config, bind_addr)?;
  Ok((endpoint, server_cert))
}
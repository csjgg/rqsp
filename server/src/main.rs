use quinn::{Connection, Endpoint, RecvStream, SendStream};
use std::error::Error;
use std::sync::Arc;
use tokio::{io, net::TcpStream, net::UdpSocket};
use uselib::{logger::init_logger, server_config::Server};

mod server_cfg;
mod sock;

#[tokio::main]
async fn main() {
    // Initialize the logger
    init_logger().await;

    // Initialize the Server configuration
    let server_config = Server::init().await;
    let bind = server_config.get_bind();

    // Start listening for connections
    let listener = match bind_connection(&bind).await {
        Ok(listener) => listener,
        Err(err) => {
            log::error!("Could not bind to {}: {}", bind, err);
            std::process::exit(1);
        }
    };
    log::info!("Listening for requests on {}", bind);

    loop {
        match listener.accept().await {
            Some(stream) => {
                tokio::spawn(async move {
                    handle_connection(stream.await.unwrap()).await;
                });
            }
            None => {
                continue;
            }
        }
    }
}

async fn bind_connection(bind: &String) -> Result<Endpoint, Box<dyn Error>> {
    let (endpoint, _server_cert) = server_cfg::make_server_endpoint(bind.parse()?)?;
    Ok(endpoint)
}

async fn handle_connection(conn: Connection) {
    log::info!("New connection:{}", conn.remote_address());
    let (mut send, mut recv) = match conn.accept_bi().await {
        Ok(s) => s,
        Err(err) => {
            log::error!("Error accepting connection: {}", err);
            return;
        }
    };
    // Here we need to impl the socks5 protocol
    let socks = match sock::handle_socks5(&mut send, &mut recv).await {
        Ok(sock) => sock,
        Err(err) => {
            log::error!("Error handling socks5: {}", err);
            return;
        }
    };
    if socks.get_conn_type() == sock::ConnectionType::TCP {
        let tar = match socks.connect_tcp(&mut send).await {
            Ok(t) => t,
            Err(err) => {
                log::error!("Error connecting to target: {}", err);
                return;
            }
        };
        if let Err(err) = forward_tcp_data(tar, &mut send, &mut recv).await {
            log::error!("Error forwarding tcp data: {}", err);
        }
    } else {
        let tar = match socks.connect_udp(&mut send).await {
            Ok(t) => t,
            Err(err) => {
                log::error!("Error connecting to target: {}", err);
                return;
            }
        };
        if let Err(err) = forward_udp_data(tar, &mut send, &mut recv).await {
            log::error!("Error forwarding udp data: {}", err);
        };
    }
    log::info!("Connection closed:{}", conn.remote_address());
}

async fn forward_tcp_data(
    tcp_stream: TcpStream,
    quinn_writer: &mut SendStream,
    quinn_reader: &mut RecvStream,
) -> io::Result<()> {
    let (mut tcp_reader, mut tcp_writer) = tcp_stream.into_split();

    let client_to_server = io::copy(&mut tcp_reader, quinn_writer);

    let server_to_client = io::copy(quinn_reader, &mut tcp_writer);

    let (client_to_server_res, server_to_client_res) =
        tokio::join!(client_to_server, server_to_client);

    match client_to_server_res {
        Ok(_) => {}
        Err(e) => {
            if e.to_string() != "connection lost" {
                return Err(e);
            }
        }
    }
    match server_to_client_res {
        Ok(_) => {}
        Err(e) => {
            if e.to_string() != "connection lost" {
                return Err(e);
            }
        }
    }
    Ok(())
}

async fn forward_udp_data(
    udp_socket: UdpSocket,
    quic_send: &mut SendStream,
    quic_recv: &mut RecvStream,
) -> io::Result<()> {
    let udp_socket = Arc::new(udp_socket);

    let udp_socket_recv = udp_socket.clone();
    let udp_socket_send = udp_socket;

    // Spawn a task for reading from UDP and writing to QUIC
    let udp_to_quic = async move {
        let mut buf = [0u8; 1024];
        loop {
            match udp_socket_recv.recv_from(&mut buf).await {
                Ok((size, _src)) => {
                    if let Err(_e) = quic_send.write_all(&buf[..size]).await {
                        return Err(tokio::io::Error::new(
                            tokio::io::ErrorKind::Other,
                            "Failed to write to QUIC stream",
                        ));
                    }
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }
    };

    // Spawn a task for reading from QUIC and writing to UDP
    let quic_to_udp = async move {
        let mut buf = [0u8; 1024];
        loop {
            match quic_recv.read(&mut buf).await {
                Ok(size) => match size {
                    Some(si) => {
                        if si == 0 {
                            return Ok(());
                        }
                        if let Err(_e) = udp_socket_send.send(&buf[..si]).await {
                            return Err(tokio::io::Error::new(
                                tokio::io::ErrorKind::Other,
                                "Failed to send to UDP socket",
                            ));
                        }
                    }
                    None => {
                        return Ok(());
                    }
                },
                Err(e) => {
                    return Err(e.into());
                }
            }
        }
    };

    // Await both tasks to handle their errors and completion
    let (udp_result, quic_result) = tokio::join!(udp_to_quic, quic_to_udp);
    udp_result?;
    quic_result?;
    Ok(())
}

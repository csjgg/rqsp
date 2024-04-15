use quinn::{Connection, Endpoint};
use tokio::{
    io,
    net::TcpStream,
    net::UdpSocket
};
use uselib::{logger::init_logger, server_config::Server};
use std::sync::Arc; 


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
    let listener = bind_connection(&bind).await;
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

async fn bind_connection(bind: &String) -> Endpoint {
    let (endpoint, _server_cert) = server_cfg::make_server_endpoint(bind.parse().unwrap()).unwrap();
    endpoint
}

async fn handle_connection(mut conn: Connection) {
    // Here we need to impl the socks5 protocol
    let socks = sock::handle_socks5(&mut conn).await.unwrap();
    if socks.get_conn_type() == sock::ConnectionType::TCP {
        let tar = socks.connect_tcp(&conn).await.unwrap();
        forward_tcp_data(tar, conn).await.unwrap();
    } else {
        let tar = socks.connect_udp(&conn).await.unwrap();
        forward_udp_data(tar, conn).await.unwrap();
    }
}

async fn forward_tcp_data(tcp_stream: TcpStream, quinn_conn: Connection) -> io::Result<()> {
    let (mut tcp_reader, mut tcp_writer) = tcp_stream.into_split();
    let (mut quinn_writer, mut quinn_reader) = quinn_conn.open_bi().await?;

    let client_to_server =
        tokio::spawn(async move { io::copy(&mut tcp_reader, &mut quinn_writer).await });

    let server_to_client =
        tokio::spawn(async move { io::copy(&mut quinn_reader, &mut tcp_writer).await });

    let (client_to_server_res, server_to_client_res) =
        tokio::join!(client_to_server, server_to_client);

    client_to_server_res??;
    server_to_client_res??;

    Ok(())
}


async fn forward_udp_data(udp_socket: UdpSocket,quic_conn: Connection) ->  io::Result<()> {
    let (mut quic_send, mut quic_recv) = quic_conn.open_bi().await?;
    let udp_socket = Arc::new(udp_socket);

    let udp_socket_recv = udp_socket.clone();
    let udp_socket_send = udp_socket;

    // Spawn a task for reading from UDP and writing to QUIC
    let udp_to_quic = tokio::spawn(async move {
        let mut buf = [0u8; 1024];
        loop {
            match udp_socket_recv.recv_from(&mut buf).await {
                Ok((size, _src)) => {
                    if let Err(_e) = quic_send.write_all(&buf[..size]).await {
                        return Err(tokio::io::Error::new(tokio::io::ErrorKind::Other, "Failed to write to QUIC stream"));
                    }
                },
                Err(e) => {
                    return Err(e);
                }
            }
        }
    });

    // Spawn a task for reading from QUIC and writing to UDP
    let quic_to_udp = tokio::spawn(async move {
        let mut buf = [0u8; 1024];
        loop {
            match quic_recv.read(&mut buf).await {
                Ok(size) => {
                    match size{
                        Some(si)=>{
                            if si == 0 {
                                return Ok(());
                            }
                            if let Err(_e) = udp_socket_send.send(&buf[..si]).await {
                                return Err(tokio::io::Error::new(tokio::io::ErrorKind::Other, "Failed to send to UDP socket"));
                            }
                        }
                        None => {
                            return Ok(());
                        }
                    }
                },
                Err(e) => {
                    return Err(e.into());
                }
            }
        }
    });

    // Await both tasks to handle their errors and completion
    let (udp_result, quic_result) = tokio::join!(udp_to_quic, quic_to_udp);
    udp_result??;
    quic_result??;
    Ok(())
}
use quinn::{Connection, Endpoint};
use std::error::Error;
use tokio::{
    io,
    net::TcpStream,
};
use uselib::{logger::init_logger, server_config::Server};

mod server_cfg;

#[tokio::main]
async fn main() {
    // Initialize the logger
    init_logger().await;

    // Initialize the Server configuration
    let server_config = Server::init().await;
    let bind = server_config.get_bind();

    // Start listening for connections
    let listener = bind_connection(&bind).await;

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
    let target = handle_socks5(&mut conn).await.unwrap();
    let targetconn = connect_to_target(&target).await.unwrap();
    if let Err(err) = forward_data(targetconn, conn).await {
        log::error!("Error forwarding data: {}", err);
    }
}

// Connect to target based on Tcp
async fn connect_to_target(target: &String) -> Result<TcpStream, Box<dyn Error>> {
    let tcp_stream = TcpStream::connect(target).await?;
    Ok(tcp_stream)
}

async fn forward_data(tcp_stream: TcpStream, quinn_conn: Connection) -> io::Result<()> {
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

async fn handle_socks5(conn: &mut Connection) -> Result<String, Box<dyn Error>> {
    Ok(" ".to_string())
}

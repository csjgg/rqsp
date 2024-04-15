use tokio::{
    io,
    net::{TcpListener, TcpStream},
};
use uselib::{
    client_config::Client,
    quic,
    logger::init_logger
};
use quinn::{Connection, Endpoint};
use std::error::Error;

#[tokio::main]
async fn main() {
    // Initialize the logger
    init_logger().await;
    // Initialize the client configuration
    let client_config = Client::init().await;
    let bind = client_config.get_bind();
    let target = client_config.get_target();
    // Start listening for connections
    let listener = bind_connection(&bind).await;
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let tar = target.clone();
                tokio::spawn(async move {
                    handle_connection(stream, &tar).await;
                });
            }
            Err(err) => {
                log::error!("Error accepting connection: {}", err);
            }
        }
    }
}

async fn bind_connection(bind: &String) -> TcpListener {
    let listener = match TcpListener::bind(bind).await {
        Ok(listener) => listener,
        Err(err) => {
            log::error!("Could not bind to {}: {}", bind, err);
            std::process::exit(1);
        }
    };
    log::info!("Listening for requests on {}", bind);
    listener
}

async fn handle_connection(conn: TcpStream, target: &String) {
    let targetconn = connect_to_target(target).await.unwrap();
    if let Err(err) = forward_data(conn, targetconn).await {
        log::error!("Error forwarding data: {}", err);
    }
}

// Connect to target based on quic
async fn connect_to_target(target: &String)->Result<Connection, Box<dyn Error>> {
    let mut endpoint = Endpoint::client("0.0.0.0:0".parse().unwrap())?;
    endpoint.set_default_client_config(quic::configure_client());

    // connect to server
    let connection = endpoint
        .connect(target.parse().unwrap(),"target")
        .unwrap()
        .await?;
    return Ok(connection);
}

async fn forward_data(tcp_stream: TcpStream, quinn_conn: Connection) -> io::Result<()> {
    let (mut tcp_reader, mut tcp_writer) = tcp_stream.into_split();
    let (mut quinn_writer, mut quinn_reader) = quinn_conn.open_bi().await?;

    let client_to_server = tokio::spawn(async move {
        io::copy(&mut tcp_reader, &mut quinn_writer).await
    });

    let server_to_client = tokio::spawn(async move {
        io::copy(&mut quinn_reader, &mut tcp_writer).await
    });

    let (client_to_server_res, server_to_client_res) = tokio::join!(client_to_server, server_to_client);

    client_to_server_res??;
    server_to_client_res??;

    Ok(())
}
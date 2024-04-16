use quinn::{RecvStream, SendStream};
use std::error::Error;
use tokio::io::{self};
use tokio::net::{TcpStream, UdpSocket};

const FIRST_RESPONSE: [u8; 2] = [0x05, 0x00];
#[derive(PartialEq, Clone)]
pub enum ConnectionType {
    TCP,
    UDP,
}

#[derive(PartialEq, Clone)]
pub enum NameType {
    IPV4,
    IPV6,
    DOMAIN,
}

pub struct Socks5 {
    conn_type: ConnectionType,
    target: String,
    port: u16,
}

pub async fn handle_socks5(send: &mut SendStream, recv: &mut RecvStream) -> Result<Socks5, Box<dyn Error>> {
    start_socks5(send, recv).await?;
    let sock = get_socks5_target(recv).await?;
    Ok(sock)
}

async fn start_socks5(send: &mut SendStream, recv: &mut RecvStream) -> Result<(), Box<dyn Error>> {
    let mut buf = [0u8; 2];
    recv.read_exact(&mut buf).await?;
    if buf[0] != 0x05 {
        return Err(Box::new(io::Error::new(
            io::ErrorKind::Other,
            "Not a socks5 protocol",
        )));
    }
    let mut nbuf = vec![0u8; buf[1] as usize];
    recv.read_exact(&mut nbuf).await?;
    send.write_all(&FIRST_RESPONSE).await?;
    Ok(())
}

async fn get_socks5_target(recv: &mut RecvStream) -> Result<Socks5, Box<dyn Error>> {
    let mut buf = [0u8; 4];
    recv.read_exact(&mut buf).await?;
    let conn = match buf[1] {
        0x01 => ConnectionType::TCP,
        0x03 => ConnectionType::UDP,
        _ => {
            return Err(Box::new(io::Error::new(
                io::ErrorKind::Other,
                "invalid connection type",
            )))
        }
    };
    let atype = match buf[3] {
        0x01 => NameType::IPV4,
        0x03 => NameType::DOMAIN,
        0x04 => NameType::IPV6,
        _ => {
            return Err(Box::new(io::Error::new(
                io::ErrorKind::Other,
                "invalid address type",
            )))
        }
    };
    let target: String;
    if atype == NameType::IPV4 {
        let mut buf = [0u8; 4];
        recv.read_exact(&mut buf).await?;
        target = format!("{}.{}.{}.{}", buf[0], buf[1], buf[2], buf[3]);
    } else if atype == NameType::DOMAIN {
        let mut length_buf = [0u8; 1];
        recv.read_exact(&mut length_buf).await?;
        let length = length_buf[0] as usize;
        let mut buf = vec![0u8; length];
        recv.read_exact(&mut buf).await?;
        target =
            String::from_utf8(buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    } else {
        let mut buf = [0u8; 16];
        recv.read_exact(&mut buf).await?;
        target = format!("{:02x}{:02x}:{:02x}{:02x}:{:02x}{:02x}:{:02x}{:02x}:{:02x}{:02x}:{:02x}{:02x}:{:02x}{:02x}:{:02x}{:02x}",
        buf[0], buf[1], buf[2], buf[3], buf[4], buf[5], buf[6], buf[7],
        buf[8], buf[9], buf[10], buf[11], buf[12], buf[13], buf[14], buf[15]);
    }
    let mut port_buf = [0u8; 2];
    recv.read_exact(&mut port_buf).await?;
    let port = u16::from_be_bytes(port_buf);
    Ok(Socks5 {
        conn_type: conn,
        target: target,
        port: port,
    })
}

impl Socks5 {
    pub fn get_conn_type(&self) -> ConnectionType {
        self.conn_type.clone()
    }
    pub async fn connect_tcp(&self, send: &mut SendStream) -> Result<TcpStream, Box<dyn Error>> {
        let stream = TcpStream::connect((self.target.as_str(), self.port)).await?;  
        let success: Vec<u8> =vec![0x05, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        send.write_all(&success).await?;
        log::info!("Connected to target{}:{}", self.target, self.port);
        Ok(stream)
    }
    pub async fn connect_udp(&self, send: &mut SendStream) -> Result<UdpSocket, Box<dyn Error>> {
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        socket.connect((self.target.as_str(), self.port)).await?;
        let success: Vec<u8> =vec![0x05, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        send.write_all(&success).await?;
        log::info!("Connected to target{}:{}", self.target, self.port);
        Ok(socket)
    }
}

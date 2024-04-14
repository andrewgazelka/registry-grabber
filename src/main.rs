use anyhow::Context;
use clap::Parser;
use tokio::{
    fs::{File, OpenOptions},
    io::AsyncWriteExt,
    net::TcpStream,
};
use valence_protocol::{
    packets, packets::handshaking::handshake_c2s::HandshakeNextState, Bounded, Packet,
    PacketDecoder, PacketEncoder, VarInt,
};

mod net;

#[derive(Parser)]
struct Args {
    #[clap(short, long, default_value = "127.0.0.1")]
    ip: String,
    #[clap(short, long, default_value = "25565")]
    port: u16,

    /// File to output registries
    #[clap(short, long, default_value = "registries.json")]
    registries_file: String,

    /// File output for tags
    #[clap(short, long, default_value = "tags.json")]
    tags_file: String,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    connect(args).await.unwrap();
}

async fn connect(args: Args) -> anyhow::Result<()> {
    let ip = args.ip;
    let port = args.port;
    let address = format!("{ip}:{port}");

    let mut stream = TcpStream::connect((ip.as_str(), port))
        .await
        .with_context(|| format!("failed to connect to {address}"))?;

    let mut encoder = PacketEncoder::new();

    // C→S: Handshake with Next State set to 2 (login)
    let pkt = packets::handshaking::HandshakeC2s {
        protocol_version: VarInt::from(net::PROTO_VERSION),
        server_address: Bounded(&ip), // todo: check length
        server_port: port,
        next_state: HandshakeNextState::Login,
    };

    encoder
        .append_packet(&pkt)
        .context("failed to encode handshake packet")?;

    stream
        .write_all(&encoder.take())
        .await
        .context("failed to write handshake packet")?;

    // C→S: Login Start
    let pkt = packets::login::LoginHelloC2s {
        username: Bounded("bob"),
        profile_id: None,
    };

    encoder
        .append_packet(&pkt)
        .context("failed to encode login packet")?;

    stream
        .write_all(&encoder.take())
        .await
        .context("failed to write login packet")?;

    let (read, _write) = stream.into_split();
    let decoder = PacketDecoder::new();

    let mut decoder = net::IoRead {
        stream: read,
        decoder,
    };
    // let byte = r
    //
    let packet = decoder
        .recv_packet_raw()
        .await
        .context("could not read raw packet")?;

    let packet: packets::login::LoginSuccessS2c = packet.decode()?;

    println!("packet: {packet:?}");

    // get BIG game packet

    let mut wrote_tags = false;
    let mut wrote_registries = false;

    loop {
        if wrote_tags && wrote_registries {
            return Ok(());
        }

        let packet = decoder
            .recv_packet_raw()
            .await
            .context("could not read raw packet")?;

        match packet.id {
            packets::play::SynchronizeTagsS2c::ID => {
                let sync_packets: packets::play::SynchronizeTagsS2c = packet.decode()?;

                let mut file = open_file(&args.tags_file).await?;
                let json = serde_json::to_vec(&sync_packets.groups)?;

                file.write_all(&json).await?;
                wrote_tags = true;
            }
            packets::play::GameJoinS2c::ID => {
                let game_join: packets::play::GameJoinS2c = packet.decode()?;
                let mut file = open_file(&args.registries_file).await?;
                let json = serde_json::to_vec(&game_join.registry_codec)?;

                file.write_all(&json).await?;
                wrote_registries = true;
            }
            _ => {}
        }
    }
}

async fn open_file(path: &str) -> anyhow::Result<File> {
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
        .await?;

    Ok(file)
}

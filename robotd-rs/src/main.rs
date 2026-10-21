use clap::Parser;
use fs2::FileExt;
use robotd::{
    control::Hub, pb::robot_control_server::RobotControlServer, server::Service, store::Store,
};
use std::{
    fs::{File, OpenOptions},
    path::PathBuf,
};

#[derive(Parser)]
#[command(version, about = "机器人统一控制、权威配置与流式音频服务")]
struct Args {
    #[arg(long, default_value = "robotd.sqlite")]
    db: PathBuf,
    #[arg(long)]
    init: Option<PathBuf>,
    #[arg(long, default_value = "127.0.0.1:50051")]
    listen: String,
    #[arg(long)]
    simulate: bool,
    #[arg(long)]
    print_default_config: bool,
}
#[tokio::main(worker_threads = 2)]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    if args.print_default_config {
        println!("{}", include_str!("../../robotd/default-config.json"));
        return Ok(());
    }
    if let Some(p) = args.db.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(p)?;
    }
    let mut lock_path = args.db.as_os_str().to_os_string();
    lock_path.push(".lock");
    let lock: File = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(lock_path)?;
    lock.try_lock_exclusive()
        .map_err(|_| anyhow::anyhow!("此数据库已由另一个 robotd 使用"))?;
    let initial = args
        .init
        .map(|p| -> anyhow::Result<_> { Ok(serde_json::from_slice(&std::fs::read(p)?)?) })
        .transpose()?;
    let store = Store::open(
        args.db
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("数据库路径无效"))?,
        initial,
    )?;
    let hub = Hub::start(store, args.simulate, tokio::runtime::Handle::current())?;
    let service = Service {
        hub: hub.clone(),
        token: std::env::var("ROBOTD_API_TOKEN").unwrap_or_default(),
    };
    eprintln!(
        "robotd Rust 已启动：{}，模拟：{}，数据库：{}",
        args.listen,
        args.simulate,
        args.db.display()
    );
    let server = tonic::transport::Server::builder()
        .max_concurrent_streams(32)
        .initial_stream_window_size(65536)
        .initial_connection_window_size(131072)
        .add_service(
            RobotControlServer::new(service)
                .max_decoding_message_size(1024 * 1024)
                .max_encoding_message_size(1024 * 1024),
        )
        .serve(args.listen.parse()?);
    tokio::pin!(server);
    let shutdown = async {
        #[cfg(unix)]
        {
            let mut term =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("注册终止信号");
            tokio::select! {_=tokio::signal::ctrl_c()=>{},_=term.recv()=>{}}
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
        }
    };
    // 状态订阅可以无限长；退出时取消 RPC，不能等客户端主动结束。
    let result = tokio::select! {
        result=&mut server=>result,
        _=shutdown=>Ok(()),
    };
    hub.shutdown().await;
    result?;
    Ok(())
}

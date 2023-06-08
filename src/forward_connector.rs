use bytes::BytesMut;
use tokio::{io::{AsyncReadExt, AsyncWriteExt}};
use std::{net::SocketAddr};

use super::config;
use super::connections;

enum ReadDirection {
	ReadRemote,
	ReadForward
}

fn update_bytes_transfered(
	transfered : usize,
	id: u64,
	connections : &mut std::sync::Arc<std::sync::Mutex<connections::Connections>>,
	direction: &ReadDirection
)
{
	let mut mtx = connections.lock().unwrap();
	let cnn = &mut *mtx;
	let mut conn = cnn.get(id);

	match direction{
		ReadDirection::ReadRemote => {
			conn.bytes_to_fowarded_ip = conn.bytes_to_fowarded_ip + transfered;
		},
		ReadDirection::ReadForward => {
			conn.bytes_to_remote_ip = conn.bytes_to_remote_ip + transfered;
		}
	};
}

async fn transfer_flow(
	read :  &mut tokio::io::ReadHalf<tokio::net::TcpStream>, 
	write : &mut tokio::io::WriteHalf<tokio::net::TcpStream>,
	id : u64,
	connections : &mut std::sync::Arc<std::sync::Mutex<connections::Connections>>,
	direction: ReadDirection,
	global_cancel_token : tokio_util::sync::CancellationToken,
	local_cancel_token : tokio_util::sync::CancellationToken
)
{
	let mut buffer = BytesMut::with_capacity(1024);
	
	loop {

		let read_size = tokio::select! {

			read_size = async {

				buffer.clear();
				let r1 = read.read_buf(&mut buffer).await;
				let size = match r1 {
					Ok(sz) => {sz},
					Err(_) => {0}
				};
				
				size
			} => {
				read_size
			},

			_ = async {
				global_cancel_token.cancelled().await;
			} => {
				return ;
			},

			_ = async {
				local_cancel_token.cancelled().await;
			} => {
				return ;
			}
		};

		if read_size == 0 {
			local_cancel_token.cancel();
			return ;
		}

		let slc = &buffer[0..read_size];
		let r2 = write.write(&slc).await;
		match r2 {
			Ok(_) => { },
			Err(_) => { return ; }
		};

		update_bytes_transfered(read_size, id, connections, &direction);
	}
	
}

fn is_whitelisted(
	cfg: &config::ForwardInfo, 
	remote_addr: &SocketAddr
) -> bool {

	if cfg.whitelist.len() == 0 {
		return true;
	}
	
	let remote_addr_str = remote_addr.ip().to_string();
	return cfg.whitelist.contains(&remote_addr_str);
}

async fn connection_flow(
	remote : tokio::net::TcpStream, 
	cfg: &config::ForwardInfo,
	connections : &mut std::sync::Arc<std::sync::Mutex<connections::Connections>>,
	cancel_token : tokio_util::sync::CancellationToken
) {

	let target = match tokio::net::TcpStream::connect(cfg.forward_to.clone()).await {
		Ok(val) => {
			val
		},
		Err(_) => {
			println!("Cannot connect to remote host");
			return;
		}
	};

	let remote_addr = remote.peer_addr().unwrap();

	if !is_whitelisted(cfg, &remote_addr){
		return ;
	}
	
	let (mut target_read, mut target_write) = tokio::io::split(target);
	let (mut remote_read, mut remote_write) = tokio::io::split(remote);

	let id = 
	{
		let mut mtx = connections.lock().unwrap();
		let cnn = &mut *mtx;
		let mut conn_info = cnn.add();
		conn_info.remote_ip = remote_addr.to_string();
		conn_info.connected_to = cfg.forward_to.clone();

		conn_info.conn_id
	};

	let local_token = tokio_util::sync::CancellationToken::new();

	let mut conn1_clone = connections.clone();
	let ct1 = cancel_token.clone();
	let lt1 = local_token.clone();
	let t1 = tokio::spawn(async move {
		transfer_flow(
			&mut target_read, 
			&mut remote_write, 
			id, 
			&mut conn1_clone, 
			ReadDirection::ReadForward, 
			ct1,
			lt1
		).await;
	});

	let mut conn2_clone = connections.clone();
	let ct2 = cancel_token.clone();
	let lt2 = local_token.clone();
	let t2 = tokio::spawn(async move {
		transfer_flow(
			&mut remote_read, 
			&mut target_write, 
			id, 
			&mut conn2_clone, 
			ReadDirection::ReadRemote, 
			ct2,
			lt2
		).await;
	});

	let mut tasks : Vec<tokio::task::JoinHandle<()>> = Vec::new();
	tasks.push(t1);
	tasks.push(t2);

	futures::future::join_all(tasks).await;

	{
		let mut mtx = connections.lock().unwrap();
		let cnn = &mut *mtx;
		cnn.get(id).active = false;
	}		
}

pub async fn do_connection_flow(
	info : config::ForwardInfo,
	cancel_token : tokio_util::sync::CancellationToken,
	connections : std::sync::Arc<std::sync::Mutex<connections::Connections>>
)
{
	let listener = tokio::net::TcpListener::bind(info.listen_to.clone()).await.unwrap();

	loop {
		let socket = tokio::select! {

			socket = async {
				let (socket, _) = listener.accept().await.unwrap();
				socket
			} => {
				socket
			},

			_ = async {
				let cl = cancel_token.clone();
				cl.cancelled().await;
			 } => {
				return ;
			}
		};

		let info_cl = info.clone();
		let mut conn_cl = connections.clone();
		let cancel_token_cl = cancel_token.clone();
		tokio::spawn(async move {
			connection_flow(socket, &info_cl, &mut conn_cl, cancel_token_cl).await;
		});
	}
}
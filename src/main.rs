use std::{io, net::SocketAddr};
use std::io::Write;
use std::collections::HashMap;
use tokio::{io::{AsyncReadExt, AsyncWriteExt}};

static CONN_ID : std::sync::Mutex<u64> = std::sync::Mutex::new(0);

fn alloc_conn_id() -> u64 {
    let mut guard = CONN_ID.lock().unwrap();
    let curr_val = *guard;
    *guard = *guard + 1;
    curr_val
}


enum ReadDirection {
	ReadRemote,
	ReadForward
}

struct ConnectionData {

    conn_id : u64,

    remote_ip            : String,
    bytes_to_remote_ip   : usize,
    bytes_to_fowarded_ip : usize,

	active: bool,
}

impl ConnectionData {
	fn new(id : u64) -> ConnectionData {
		return ConnectionData {
			conn_id : id,

			remote_ip : "".to_string(),
			bytes_to_remote_ip : 0,
			bytes_to_fowarded_ip : 0,

			active : true,
		};
	}
}

struct Connections {
    connections : HashMap<u64, ConnectionData>,
}

impl Connections {

	fn new () -> Connections {
		return Connections { 
			connections: HashMap::new() 
		};
	}

	fn add(&mut self, id : u64) {
	
		match self.connections.get(&id){
			Some(_) => {
				panic!("Already contains this id");
			}
			None => {}
		};

		let cn = ConnectionData::new(id);
		self.connections.insert(id, cn);
	}

	fn get<'a>(&'a mut self, id : u64) -> Option<&'a mut ConnectionData> {
		return self.connections.get_mut(&id);
	}

}

struct Config {
    listen_to : String,
    forward_to : String,
    whitelist : Option<Vec<String>>
}

fn show_help() {
    println!("PortForwarder allows to forward a port to a remote destination");
    println!("Available options: ");
    println!("help -- shows this help");
    println!("stats -- shows statistics about the connections");
    println!("exit -- terminates all connections and exit");
}

fn get_command() -> String {
    print!("> ");
    std::io::stdout().flush().expect("Error flushing");

    let mut buffer = String::new();
	match io::stdin().read_line(&mut buffer) {
		Ok(_) => {},
		Err(_) => {
            panic!("Error on command!");
        },
	}

    buffer = buffer.trim_end().to_string();
    return buffer;
}

fn show_stats(connections : &std::sync::Arc<std::sync::Mutex<Connections>>) 
{
	let mtx = connections.lock().unwrap();
	let cnn = &*mtx;

	for value in cnn.connections.values(){
		println!("[{}]. Active: {}. Remote ip: {}", value.conn_id, value.active, value.remote_ip);
		println!("Bytes remote to fwd: {}", value.bytes_to_fowarded_ip);
		println!("Bytes fwd to remote: {}", value.bytes_to_remote_ip);
	}
}

fn update_bytes_transfered(
	transfered : usize,
	id: u64,
	connections : &mut std::sync::Arc<std::sync::Mutex<Connections>>,
	direction: &ReadDirection
)
{
	let mut mtx = connections.lock().unwrap();
	let cnn = &mut *mtx;
	let mut conn = cnn.get(id).unwrap();

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
	connections : &mut std::sync::Arc<std::sync::Mutex<Connections>>,
	direction: ReadDirection,
	cancel_token : tokio_util::sync::CancellationToken
)
{
	let mut buf = vec![0; 1024];

	loop {

		let read_size = tokio::select! {

			read_size = async {

				let r1 = read.read_buf(&mut buf).await;
				let size = match r1 {
					Ok(sz) => {sz},
					Err(_) => {0}
				};
				
				size
			} => {
				read_size
			},

			_ = async {
				cancel_token.cancelled().await;
			} => {
				return ;
			}
		};

		if read_size == 0 {
			return ;
		}

		let slc = &buf[0..read_size];
		let r2 = write.write(&slc).await;
		match r2 {
			Ok(_) => { },
			Err(_) => { return ; }
		};

		update_bytes_transfered(read_size, id, connections, &direction);
	}
	
}

fn is_whitelisted(
	cfg: &Config, 
	remote_addr: &SocketAddr
) -> bool {

	let wls = match &cfg.whitelist {
		None => {return true;},
		Some(x) => {x}
	};

	return true;
}

async fn connection_flow(
	remote : tokio::net::TcpStream, 
	cfg: &Config,
	connections : &mut std::sync::Arc<std::sync::Mutex<Connections>>,
	cancel_token : tokio_util::sync::CancellationToken
) {

	let target = match tokio::net::TcpStream::connect(cfg.forward_to.clone()).await {
        Ok(val) => {
            val
        },
        Err(_) => {
            println!("Cannot connet to remote host");
            return;
        }
    };

	let remote_addr = remote.peer_addr().unwrap();

	if !is_whitelisted(cfg, &remote_addr){
		return ;
	}
	
	let (mut target_read, mut target_write) = tokio::io::split(target);
	let (mut remote_read, mut remote_write) = tokio::io::split(remote);

	let id = alloc_conn_id();
	{
		let mut mtx = connections.lock().unwrap();
		let cnn = &mut *mtx;
		cnn.add(id);
		cnn.get(id).unwrap().remote_ip = remote_addr.to_string();
	}

	let mut conn1_clone = connections.clone();
	let ct1 = cancel_token.clone();
	let t1 = tokio::spawn(async move {
		transfer_flow(&mut target_read, &mut remote_write, id, &mut conn1_clone, ReadDirection::ReadForward, ct1).await;
	});

	let mut conn2_clone = connections.clone();
	let ct2 = cancel_token.clone();
	let t2 = tokio::spawn(async move {
		transfer_flow(&mut remote_read, &mut target_write, id, &mut conn2_clone, ReadDirection::ReadRemote, ct2).await;
	});

	let mut tasks : Vec<tokio::task::JoinHandle<()>> = Vec::new();
	tasks.push(t1);
	tasks.push(t2);

	for task in tasks {
		task.await.unwrap();
	}

	{
		let mut mtx = connections.lock().unwrap();
		let cnn = &mut *mtx;
		cnn.get(id).unwrap().active = false;
	}	
	
}

async fn worker_thread_async(
	cfg : Config,
	connections : &mut std::sync::Arc<std::sync::Mutex<Connections>>,
	cancel_token : tokio_util::sync::CancellationToken
)  {   
    
	let cancel_token_cp = cancel_token.clone();
	let cancel_token_cp2 = cancel_token.clone();
	

	let listener = tokio::select! {

		lst = async {
			let listener = tokio::net::TcpListener::bind(cfg.listen_to.clone()).await.unwrap();
			listener
		} => {
			lst
		},

		_ = async {
			cancel_token_cp.cancelled().await;
		} => {
			return ;
		}
	};

	let config_arc : std::sync::Arc<Config> = std::sync::Arc::new(cfg);

	loop {
				
		let socket = tokio::select! {

			socket = async {
				let (socket, _) = listener.accept().await.unwrap();
				socket
			} => {
				socket
			},

			_ = async {
				cancel_token_cp2.cancelled().await;
			 } => {
				return ;
			}
		};

		let mut cn = connections.clone();
		let config_arc_cln = config_arc.clone();
		let cancel_token_cp3 = cancel_token.clone();
		connection_flow(socket, &config_arc_cln, &mut cn, cancel_token_cp3).await;

		if cancel_token.is_cancelled() {
			break;
		}
	}

}

fn worker_thread(
	cfg : Config, 
	mut connections : &mut std::sync::Arc<std::sync::Mutex<Connections>>,
	cancel_token : tokio_util::sync::CancellationToken
){
    
    tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()
        .unwrap()
        .block_on(worker_thread_async(cfg, &mut connections, cancel_token));
}

fn main() {

    let cfg = Config {
        listen_to : "0.0.0.0:8080".to_string(),
        forward_to : "google.com:80".to_string(),
        whitelist : None
    };

	let cancel_token = tokio_util::sync::CancellationToken::new();
	let mut connections : std::sync::Arc<std::sync::Mutex<Connections>> = std::sync::Arc::new(std::sync::Mutex::new(Connections::new()));

	let connections2 = connections.clone();
	let cancel_token2 = cancel_token.clone();

    let th = std::thread::spawn(move || {
        worker_thread(cfg, &mut connections, cancel_token2);
    });

    loop {
        let cmd = get_command();
        match cmd.as_str() {
            "exit" => break,
            "help" => {
                show_help();
            },
			"stats" => {
				show_stats(&connections2);
			}
            _ => {
                show_help();
            }
        }
    }

	cancel_token.cancel();
	th.join().unwrap();
    
}

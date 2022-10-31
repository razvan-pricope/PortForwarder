use std::io;
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

struct ConnectionData {

    conn_id : u64,

    remote_ip            : String,
    bytes_to_remote_ip   : u64,
    bytes_to_fowarded_ip : u64,

	active: bool
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
    println!("stas -- shows statistics about the connections");
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

async fn transfer_flow(
	read :  &mut tokio::io::ReadHalf<tokio::net::TcpStream>, 
	write : &mut tokio::io::WriteHalf<tokio::net::TcpStream>
)  -> usize
{
	let mut buf = vec![0; 1024];
	
	let r1 = read.read_buf(&mut buf).await;
	let size = match r1 {
		Ok(sz) => {
			sz
		},
		Err(_) => { return 0; }
	};

	let slc = &buf[0..size];
	let r2 = write.write(&slc).await;
	match r2 {
		Ok(sz) => { return sz;},
		Err(_) => { return 0; }
	};
	
}

async fn transfer_flow_remote_to_forward(
	mut read :  tokio::io::ReadHalf<tokio::net::TcpStream>, 
	mut write : tokio::io::WriteHalf<tokio::net::TcpStream>,
)
{
	loop {
		let r = transfer_flow(&mut read, &mut write).await;
		if r == 0 {
			break;
		}
	}
}

async fn connection_flow(
	remote : tokio::net::TcpStream, 
	cfg: &Config,
	connections : &mut std::sync::Arc<std::sync::Mutex<Connections>>
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

	let remote_ip = remote.peer_addr().unwrap().to_string();
	let (target_read, target_write) = tokio::io::split(target);
	let (remote_read, remote_write) = tokio::io::split(remote);

	let id = alloc_conn_id();
	{
		let mut mtx = connections.lock().unwrap();
		let cnn = &mut *mtx;
		cnn.add(id);
		cnn.get(id).unwrap().remote_ip = remote_ip;
	}

	let t1 = tokio::spawn(async move {
		transfer_flow_remote_to_forward(target_read, remote_write).await;
	});

	let t2 = tokio::spawn(async move {
		transfer_flow_remote_to_forward(remote_read, target_write).await;
	});

	loop {
		tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

		if t1.is_finished() || t2.is_finished(){
			break;
		}
	}

	{
		let mut mtx = connections.lock().unwrap();
		let cnn = &mut *mtx;
		cnn.add(id);
		cnn.get(id).unwrap().active = false;
	}	
	
}

async fn worker_thread_async(
	cfg : Config,
	mut connections : &mut std::sync::Arc<std::sync::Mutex<Connections>>
) -> Result<(), Box<dyn std::error::Error>> {   
    
    let listener = tokio::net::TcpListener::bind(cfg.listen_to.clone()).await?;

    loop {
        let (socket, _) = listener.accept().await?;
        connection_flow(socket, &cfg, &mut connections).await;
    }
}

fn worker_thread(cfg : Config, mut connections : &mut std::sync::Arc<std::sync::Mutex<Connections>>){
    
    let r = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()
        .unwrap()
        .block_on(worker_thread_async(cfg, &mut connections));

    match r {
        Ok(_) => {},
        Err(_) => {
            panic!("Cannot run tokio runtime");
        }
    }
}

fn main() {

    let cfg = Config {
        listen_to : "0.0.0.0:8080".to_string(),
        forward_to : "google.com:80".to_string(),
        whitelist : None
    };

	let mut connections : std::sync::Arc<std::sync::Mutex<Connections>> = std::sync::Arc::new(std::sync::Mutex::new(Connections::new()));

    std::thread::spawn(move || {
        worker_thread(cfg, &mut connections);
    });

    loop {
        let cmd = get_command();
        match cmd.as_str() {
            "exit" => break,
            "help" => {
                show_help();
            },
            _ => {
                show_help();
            }
        }
    }
    
}

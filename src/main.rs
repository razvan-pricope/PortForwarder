use std::io::Write;

mod config;
mod connections;
mod forward_manager;
mod forward_connector;



fn show_help() {
	println!("PortForwarder allows to forward a port to a remote destination");
	println!("port_forwarder.exe --config path_to_json.jso");
	println!("Available options: ");
	println!("help  -- shows this help");
	println!("stats -- shows statistics about active connections");
	println!("hist  -- historical stats about closed connections");
	println!("exit  -- terminates all connections and exit");
}

fn get_command() -> String {
	print!("> ");
	std::io::stdout().flush().expect("Error flushing");

	let mut buffer = String::new();
	match std::io::stdin().read_line(&mut buffer) {
		Ok(_) => {},
		Err(_) => {
			panic!("Error on command!");
		},
	}

	buffer = buffer.trim_end().to_string();
	return buffer;
}

fn show_stats(
	connections : &std::sync::Arc<std::sync::Mutex<connections::Connections>>, 
	active : bool) 
{
	let mtx = connections.lock().unwrap();
	let cnn = &*mtx;

	for value in cnn.connections.values().filter(|&cd| active == cd.active){
		println!("[{}]", value.conn_id);
		println!("Remote ip: {}", value.remote_ip);
		println!("Forward: {}", value.connected_to);
		println!("Bytes remote -> fwd: {}", value.bytes_to_fowarded_ip);
		println!("Bytes fwd -> remote: {}", value.bytes_to_remote_ip);
	}
}

fn get_config_file() -> Option<String> {
	let args : Vec<_> = std::env::args().collect();

	if args.len() < 2 {
		return None;
	}

	if !args[1].eq("--config") {
		return None;
	}

	return Some(args[2].clone());
}

fn get_config() -> Option<config::Config> {

	let file = get_config_file()?;
	return config::Config::from_file(&file);
}

fn main() {

	let cfg = match get_config() {
		None => {
			show_help();
			return ;
		},
		Some(x) => {x}
	};

	let mut fwd = forward_manager::Forwarder::start(cfg);

	loop {
		let cmd = get_command();
		match cmd.as_str() {
			"exit" => break,
			"help" => {
				show_help();
			},
			"stats" => {
				show_stats(&fwd.get_connections(), true);
			},
			"hist" => {
				show_stats(&fwd.get_connections(), false);
			},
			_ => {
				show_help();
			}
		}
	}

	fwd.stop();
	return ();
}

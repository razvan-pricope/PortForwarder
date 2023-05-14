
use std::sync::{Arc, Mutex};

//#[path = "config.rs"] mod config;
use super::config;
use super::connections;
use super::forward_connector;

struct ThreadData {
	config : config::Config,

	cancel_token : tokio_util::sync::CancellationToken,
	connections : Arc<Mutex<connections::Connections>>,
}

pub struct Forwarder {
	shared_data : Arc<Mutex<ThreadData>>,
	worker : Option<std::thread::JoinHandle<()>>,
}

impl Forwarder {

	pub fn start(cfg : config::Config) -> Forwarder {

		let data = ThreadData {
			config : cfg,
			cancel_token : tokio_util::sync::CancellationToken::new(),
			connections : Arc::new(Mutex::new(connections::Connections::new()))
		};
		
		let shared_data : Arc<Mutex<ThreadData>> = Arc::new(Mutex::new(data));
		let shared_data_1 = shared_data.clone();
		let worker = std::thread::spawn(move || {
			worker_thread(shared_data_1)
		});
		
		return Forwarder {
			shared_data : shared_data,
			worker : Some(worker)
		};
	}

	pub fn stop(&mut self) { 
		let shared_data = &*(self.shared_data.lock().unwrap());
		shared_data.cancel_token.cancel();
		drop(shared_data);

		if let Some(data) = self.worker.take() {
			data.join().unwrap();
		}
	}
	
	pub fn get_connections(&self) -> Arc<Mutex<connections::Connections>> {
		let shared_data = &*(self.shared_data.lock().unwrap());
		return shared_data.connections.clone();
	}
}

async fn listener_thread_async(shared_data : Arc<Mutex<ThreadData>>, idx_cfg : usize) {

	let tmp = {
		let shared_data_ = shared_data.lock().unwrap();
		let cancel_token_cp = shared_data_.cancel_token.clone();
		let listener_info = shared_data_.config.forward_info.get(idx_cfg).unwrap().clone();
		let conn = shared_data_.connections.clone();
		(cancel_token_cp, listener_info, conn)
	};

	let cancel_token   = tmp.0;
	let listener_info  = tmp.1;
	let conn           = tmp.2;

	forward_connector::do_connection_flow(listener_info, cancel_token, conn).await;
}

async fn worker_thread_async(shared_data : Arc<Mutex<ThreadData>>) {
	
	let shared_data_ = shared_data.lock().unwrap();
	let total_listeners = shared_data_.config.forward_info.len();
	drop(shared_data_);

	let mut tasks : Vec<tokio::task::JoinHandle<()>> = Vec::new();

	for i in 0 .. total_listeners {

		let shared_data_cp = shared_data.clone();
		let th = tokio::spawn(async move {
			listener_thread_async(shared_data_cp, i).await;
		});

		tasks.push(th);
	}

	futures::future::join_all(tasks).await;
}

fn worker_thread(data : Arc<Mutex<ThreadData>>) {
    tokio::runtime::Builder::new_current_thread()
		.enable_io()
		.build()
		.unwrap()
		.block_on(worker_thread_async(data));
}

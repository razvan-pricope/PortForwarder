
//#[path = "config.rs"] mod config;
use super::config;
use super::connections;
use super::forward_connector;

struct ThreadData {
	config : config::Config,

	cancel_token : tokio_util::sync::CancellationToken,
	connections : std::sync::Arc<std::sync::Mutex<connections::Connections>>,
}

pub struct ForwardManager {
	shared_data : std::sync::Arc<std::sync::Mutex<ThreadData>>,
	worker : Option<std::thread::JoinHandle<()>>,
}

impl ForwardManager {

	pub fn start(cfg : config::Config) -> ForwardManager {

		let data = ThreadData {
			config : cfg,
			cancel_token : tokio_util::sync::CancellationToken::new(),
			connections : std::sync::Arc::new(std::sync::Mutex::new(connections::Connections::new()))
		};
		
		let shared_data : std::sync::Arc<std::sync::Mutex<ThreadData>> = std::sync::Arc::new(std::sync::Mutex::new(data));
		let shared_data_1 = shared_data.clone();
		let worker = std::thread::spawn(move || {
			worker_thread(shared_data_1)
		});
		
		return ForwardManager {
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
	
	pub fn get_connections(&self) -> std::sync::Arc<std::sync::Mutex<connections::Connections>> {
		let shared_data = &*(self.shared_data.lock().unwrap());
		return shared_data.connections.clone();
	}
}

async fn listener_thread_async(shared_data : std::sync::Arc<std::sync::Mutex<ThreadData>>, idx_cfg : usize) {

	let tmp = {
		let shared_data_ = shared_data.lock().unwrap();
		let cancel_token_cp = shared_data_.cancel_token.clone();
		let listener_info = shared_data_.config.forward_info.get(idx_cfg).unwrap().clone();
		let connections = shared_data_.connections.clone();
		(cancel_token_cp, listener_info, connections)
	};

	let cancel_token   = tmp.0;
	let listener_info  = tmp.1;
	let connections    = tmp.2;

	forward_connector::do_connection_flow(clistener_info, cancel_token, connections).await.unwrap();
}

async fn worker_thread_async(shared_data : std::sync::Arc<std::sync::Mutex<ThreadData>>) {
	
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

fn worker_thread(data : std::sync::Arc<std::sync::Mutex<ThreadData>>) {
    tokio::runtime::Builder::new_current_thread()
		.enable_io()
		.build()
		.unwrap()
		.block_on(worker_thread_async(data));
}

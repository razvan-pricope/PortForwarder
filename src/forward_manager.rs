
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
	tokio_data : Option< (tokio::runtime::Runtime, tokio::task::JoinHandle<()> ) >, 
}

impl Forwarder {

	pub fn start(cfg : config::Config) -> Forwarder {

		let data = ThreadData {
			config : cfg,
			cancel_token : tokio_util::sync::CancellationToken::new(),
			connections : Arc::new(Mutex::new(connections::Connections::new()))
		};
		
		let shared_data : Arc<Mutex<ThreadData>> = Arc::new(Mutex::new(data));
		let t_rt = start_runtime(&shared_data);
		
		return Forwarder {
			shared_data : shared_data,
			tokio_data : Some(t_rt)
		};
	}

	pub fn stop(&mut self) { 

		{
			let shared_data = &*(self.shared_data.lock().unwrap());
			shared_data.cancel_token.cancel();
		}

		if let Some(data) = self.tokio_data.take() {
			let (_, t_jh) = data;
			futures::executor::block_on(t_jh).unwrap();
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
	
	let total_listeners = {
		let shared_data_ = shared_data.lock().unwrap();
		let total_listeners = shared_data_.config.forward_info.len();
		total_listeners
	};

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

fn start_runtime(data : &Arc<Mutex<ThreadData>>) -> (tokio::runtime::Runtime, tokio::task::JoinHandle<()>)
{
	let rt = tokio::runtime::Builder::new_multi_thread()
		.enable_all()
		.build()
		.unwrap();
	
	let cl = data.clone();
	let jh = rt.spawn(async {
		worker_thread_async(cl).await;
	});

	(rt, jh)
}